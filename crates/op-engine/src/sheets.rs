//! Комплект листов из модели: обмерный план, план демонтажа, план монтажа
//! перегородок и план помещений после перепланировки.
//!
//! Все листы комплекта — в одном масштабе и с одним положением плана, чтобы
//! листы совпадали при наложении. Масштаб подбирается по самому тесному листу
//! (не крупнее 1:50) или берётся из настроек чертежей. Таблицы и легенды стоят
//! справа от плана над основной надписью или под планом — где плану просторнее;
//! что не поместилось, переносится на листы продолжения.

mod climate;
mod elevations;
mod finishes;
mod plumbing;

use std::collections::{HashMap, HashSet};

use glam::DVec2;
use op_export::{
    Align, ExportError, Item, Rgb, Sheet, Text, TitleBlock, Viewport,
    font::SheetFont,
    frame::{self, TITLE_HEIGHT, TITLE_WIDTH},
    hatch::hatch,
    pdf::{self, PdfInfo},
    safe_file_name,
    sheet::Shape as Mark,
    svg::{self, SvgOptions},
    table::{Cell, Column, THICK, THIN, Table, fit_height},
    xlsx::{self, Worksheet},
};
use op_geom::shapes::{self, Shape};
use op_model::{Document, OpeningKind, Wall, WallMaterial, view::demolished};

use crate::{
    derived::Derived,
    draw::{Pen, opening_lines},
    electric,
    geometry::{self, DimensionGeometry},
    protocol::{DrawingFormat, DrawingSetInfo, ExportNames, SheetPreview},
    variants::view_name,
};

/// Масштабы по ГОСТ 2.302, из которых выбирается масштаб плана.
pub const SCALES: [u32; 11] = [20, 25, 40, 50, 75, 100, 200, 400, 500, 800, 1000];
/// Самый крупный масштаб, который подбирается автоматически.
const AUTO_LARGEST: u32 = 50;
/// Поле вокруг плана внутри видового экрана, мм бумаги (подписи, размеры).
const PLAN_MARGIN: f64 = 8.0;
/// Полоса над планом с его названием, мм.
const CAPTION_BAND: f64 = 10.0;
/// Зазор между таблицами и до рамки, мм.
const GAP: f64 = 4.0;
/// Наименьшая ширина плана слева от таблиц, мм.
const MIN_PLAN_WIDTH: f64 = 80.0;
/// Доля высоты листа, которую таблицы могут занять под планом.
const STACK_SHARE: f64 = 0.45;

/// Высота названия помещения, мм бумаги (ГОСТ 2.304).
const NAME_HEIGHT: f64 = 3.5;
/// Высота мелкого текста: площади, размеры, выноски, марки.
const SMALL_HEIGHT: f64 = 2.5;
/// Контур стен, мм.
const WALL_LINE: f64 = THICK;
/// Линии символов проёмов, мм.
const SYMBOL_LINE: f64 = 0.25;
/// Линии штриховки, мм.
const HATCH_LINE: f64 = 0.13;

/// Заливка существующих стен.
const WALL_FILL: Rgb = Rgb::hex(0xBDBDBD);
/// Демонтаж.
const DEMOLISH: Rgb = Rgb::hex(0xC2410C);
/// Штрих демонтируемых конструкций, мм.
const DEMOLISH_DASH: (f64, f64) = (2.0, 1.0);

// Слои листа (они же слои PDF и DXF).
const LAYER_FRAME: &str = "Рамка и штамп";
const LAYER_TABLES: &str = "Таблицы";
const LAYER_WALLS: &str = "Стены";
const LAYER_OPENINGS: &str = "Проёмы";
const LAYER_ROOMS: &str = "Помещения";
const LAYER_DIMENSIONS: &str = "Размеры";
const LAYER_NOTES: &str = "Выноски";
const LAYER_DEMOLITION: &str = "Демонтаж";
const LAYER_CONSTRUCTION: &str = "Монтаж";
const LAYER_FURNITURE: &str = "Мебель";
const LAYER_ELECTRIC: &str = "Электрика";

/// Лист комплекта.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Survey,
    Demolition,
    Construction,
    Rooms,
    Furniture,
    Sockets,
    Lighting,
    Switches,
    Wiring,
    Water,
    Sewer,
    Floors,
    Ceilings,
    Climate,
}

impl Kind {
    fn title(self) -> &'static str {
        match self {
            Self::Survey => "Обмерный план",
            Self::Demolition => "План демонтажа",
            Self::Construction => "План монтажа перегородок",
            Self::Rooms => "План помещений",
            Self::Furniture => "План расстановки мебели",
            Self::Sockets => "План розеток и выводов",
            Self::Lighting => "План освещения",
            Self::Switches => "План выключателей",
            Self::Wiring => "План электропроводки",
            Self::Water => "План водоснабжения",
            Self::Sewer => "План канализации",
            Self::Floors => "План полов",
            Self::Ceilings => "План потолков",
            Self::Climate => "План тёплых полов и климата",
        }
    }
}

/// Комплект листов и таблицы для XLSX.
#[derive(Debug, Clone)]
pub struct SheetSet {
    /// Листы по порядку.
    pub sheets: Vec<Sheet>,
    /// Ведомости для XLSX.
    pub worksheets: Vec<Worksheet>,
    /// Знаменатель масштаба планов.
    pub scale: u32,
    /// Масштаб подобран автоматически.
    pub auto_scale: bool,
    /// Предупреждения для пользователя.
    pub warnings: Vec<String>,
    /// Основа имён файлов: «Проект — Вариант А».
    pub name: String,
    /// Автор для свойств PDF.
    pub author: String,
}

/// Собирает комплект листов вида `variant` (`None` — только обмер).
/// `date` — дата для основной надписи (ММ.ГГ).
#[must_use]
pub fn build(doc: &Document, variant: Option<&str>, level: Option<&str>, date: &str) -> SheetSet {
    let views = Views::new(doc, variant, level);
    let mut kinds = vec![Kind::Survey];
    if views.variant.is_some() {
        if !views.demolition_rows().is_empty() {
            kinds.push(Kind::Demolition);
        }
        if !views.construction_rows().is_empty() {
            kinds.push(Kind::Construction);
        }
        kinds.push(Kind::Rooms);
    }
    if !views.furniture_rows().is_empty() {
        kinds.push(Kind::Furniture);
    }
    {
        let (doc, derived) = views.final_view();
        let has = |f: &dyn Fn(op_model::DeviceKind) -> bool| {
            doc.devices
                .iter()
                .any(|d| derived.visible.contains(&d.layer) && f(d.kind))
        };
        if has(&electric::is_outlet) {
            kinds.push(Kind::Sockets);
        }
        if has(&op_model::DeviceKind::is_light) {
            kinds.push(Kind::Lighting);
        }
        if has(&electric::is_switch) {
            kinds.push(Kind::Switches);
        }
        if !doc.circuits.is_empty() && has(&|_| true) {
            kinds.push(Kind::Wiring);
        }
    }
    if views.has_plumbing(true) {
        kinds.push(Kind::Water);
    }
    if views.has_plumbing(false) {
        kinds.push(Kind::Sewer);
    }
    if views.has_finishes(op_model::Surface::Floor) {
        kinds.push(Kind::Floors);
    }
    if views.has_finishes(op_model::Surface::Ceiling) {
        kinds.push(Kind::Ceilings);
    }
    if views.has_climate() {
        kinds.push(Kind::Climate);
    }

    let size = {
        let (w, h) = doc.drawing.paper.size();
        DVec2::new(w, h)
    };
    let mut warnings = Vec::new();
    let (center, extent) = match views.extent() {
        Some((lo, hi)) => (lo.midpoint(hi), (hi - lo).max(DVec2::splat(1000.0))),
        None => {
            warnings.push("План пуст: нарисуйте стены, чтобы они попали на листы.".to_owned());
            (DVec2::ZERO, DVec2::splat(1000.0))
        }
    };

    let placements: Vec<(Kind, Placement)> = kinds
        .iter()
        .map(|&kind| (kind, place(size, views.tables(kind), extent)))
        .collect();
    let needed = placements
        .iter()
        .map(|(_, p)| needed_denominator(p.plan, extent))
        .fold(0.0, f64::max);
    let (scale, auto_scale) = match doc.drawing.scale {
        Some(scale) => {
            if f64::from(scale) < needed * (1.0 - 1e-9) {
                warnings.push(format!(
                    "План не помещается на лист в масштабе 1:{scale}: выберите более мелкий масштаб или больший формат."
                ));
            }
            (scale, false)
        }
        None => (pick_scale(needed), true),
    };
    let k = f64::from(scale);

    let mut pages: Vec<Sheet> = Vec::new();
    for (kind, placement) in placements {
        let mut sheet = new_sheet(kind.title(), size);
        let tables_layer = sheet.layer(LAYER_TABLES);
        let (lo, hi) = placement.plan;
        let clip = (lo + DVec2::new(0.0, CAPTION_BAND), hi);
        sheet.paper.push(Item::text(
            tables_layer,
            Text::centered(
                DVec2::new(lo.x.midpoint(hi.x), lo.y + CAPTION_BAND / 2.0),
                format!("{} (М 1:{scale})", kind.title()),
                NAME_HEIGHT,
                Align::Center,
            ),
        ));
        let mut plot = Plot {
            sheet: &mut sheet,
            items: Vec::new(),
            k,
        };
        views.draw(kind, &mut plot);
        let items = plot.items;
        sheet.viewports.push(Viewport {
            items,
            center,
            origin: clip.0.midpoint(clip.1),
            denominator: k,
            clip,
        });
        for (table, at) in &placement.tables {
            table.draw(tables_layer, *at, &mut sheet.paper);
        }
        pages.push(sheet);
        for (i, tables) in continuation(size, placement.overflow)
            .into_iter()
            .enumerate()
        {
            // Названия листов различаются: по ним UI различает листы в списке.
            let title = match i {
                0 => format!("{} (продолжение)", kind.title()),
                _ => format!("{} (продолжение {})", kind.title(), i + 1),
            };
            let mut sheet = new_sheet(&title, size);
            let layer = sheet.layer(LAYER_TABLES);
            for (table, at) in &tables {
                table.draw(layer, *at, &mut sheet.paper);
            }
            pages.push(sheet);
        }
    }

    // Развёртки стен итогового вида и площади стен.
    let elevation_rooms = elevations::rooms(&views);
    let walls_table = elevations::walls_table(&elevation_rooms);
    pages.extend(elevations::sheets(
        &elevation_rooms,
        walls_table.clone(),
        size,
    ));
    let mut worksheets = views.worksheets();
    if let Some(table) = walls_table {
        worksheets.push(Worksheet {
            name: "Стены".into(),
            tables: vec![table],
        });
    }

    let mut view = view_name(doc, views.variant.as_ref().map(|v| v.id.as_str()));
    // С этажами — название этажа в штампе и именах файлов.
    if !doc.levels.is_empty() {
        view = format!("{} · {view}", crate::variants::level_name(doc, level));
    }
    let building = if doc.meta.title.trim().is_empty() {
        view.clone()
    } else {
        format!("{} · {view}", doc.meta.title.trim())
    };
    let count = u32::try_from(pages.len()).unwrap_or(u32::MAX);
    for (i, sheet) in pages.iter_mut().enumerate() {
        let block = TitleBlock {
            designation: doc.meta.designation.clone(),
            object: doc.meta.address.clone(),
            building: building.clone(),
            sheet_title: sheet.title.clone(),
            stage: doc.meta.stage.clone(),
            sheet: u32::try_from(i + 1).unwrap_or(u32::MAX),
            sheets: count,
            organization: doc.meta.organization.clone(),
            developer: doc.meta.developer.clone(),
            checker: doc.meta.checker.clone(),
            date: date.to_owned(),
        };
        let layer = sheet.layer(LAYER_FRAME);
        frame::draw(size, &block, layer, &mut sheet.paper);
    }

    SheetSet {
        sheets: pages,
        worksheets,
        scale,
        auto_scale,
        warnings,
        name: safe_file_name(&if doc.meta.title.trim().is_empty() {
            view
        } else {
            format!("{} — {view}", doc.meta.title.trim())
        }),
        author: if doc.meta.developer.trim().is_empty() {
            doc.meta.organization.clone()
        } else {
            doc.meta.developer.clone()
        },
    }
}

fn new_sheet(title: &str, size: DVec2) -> Sheet {
    let mut sheet = Sheet {
        title: title.to_owned(),
        size,
        layers: Vec::new(),
        paper: Vec::new(),
        viewports: Vec::new(),
    };
    sheet.layer(LAYER_FRAME);
    sheet
}

impl SheetSet {
    /// Сведения для окна «Чертежи»: листы в SVG (текст контурами), масштаб, предупреждения.
    #[must_use]
    pub fn preview(&self) -> DrawingSetInfo {
        DrawingSetInfo {
            sheets: self
                .sheets
                .iter()
                .map(|s| SheetPreview {
                    title: s.title.clone(),
                    width: s.size.x,
                    height: s.size.y,
                    svg: svg::write(
                        s,
                        SvgOptions {
                            text_as_paths: true,
                            embed_font: false,
                        },
                    ),
                })
                .collect(),
            scale: self.scale,
            auto_scale: self.auto_scale,
            warnings: self.warnings.clone(),
            name: self.name.clone(),
            file_names: ExportNames {
                pdf: self.file_name(DrawingFormat::Pdf),
                svg: self.file_name(DrawingFormat::Svg),
                dxf: self.file_name(DrawingFormat::Dxf),
                xlsx: self.file_name(DrawingFormat::Xlsx),
                bundle: self.file_name(DrawingFormat::Bundle),
                ifc: self.file_name(DrawingFormat::Ifc),
            },
        }
    }

    /// Расширение файла экспорта: несколько листов SVG или DXF уходят в ZIP.
    #[must_use]
    pub fn extension(&self, format: DrawingFormat) -> &'static str {
        match format {
            DrawingFormat::Pdf => "pdf",
            DrawingFormat::Xlsx => "xlsx",
            DrawingFormat::Ifc => "ifc",
            DrawingFormat::Svg if self.sheets.len() == 1 => "svg",
            DrawingFormat::Dxf if self.sheets.len() == 1 => "dxf",
            DrawingFormat::Svg | DrawingFormat::Dxf | DrawingFormat::Bundle => "zip",
        }
    }

    /// Имя файла экспорта.
    #[must_use]
    pub fn file_name(&self, format: DrawingFormat) -> String {
        let suffix = match format {
            DrawingFormat::Xlsx => " — ведомости",
            DrawingFormat::Ifc => " — модель",
            DrawingFormat::Bundle => " — пакет для бригады",
            _ => "",
        };
        format!("{}{suffix}.{}", self.name, self.extension(format))
    }

    /// Файл экспорта.
    ///
    /// # Errors
    ///
    /// Встроенный шрифт не читается или ошибка упаковки.
    pub fn export(&self, format: DrawingFormat) -> Result<Vec<u8>, ExportError> {
        let per_sheet = |ext: &str, write: &dyn Fn(&Sheet) -> Vec<u8>| {
            let files: Vec<(String, Vec<u8>)> = self
                .sheets
                .iter()
                .enumerate()
                .map(|(i, s)| {
                    (
                        format!("{:02} {}.{ext}", i + 1, safe_file_name(&s.title)),
                        write(s),
                    )
                })
                .collect();
            match files.as_slice() {
                [(_, bytes)] => Ok(bytes.clone()),
                _ => op_export::zip(&files),
            }
        };
        match format {
            // IFC пишется из модели, а не из листов (`EngineState::export_drawings`).
            DrawingFormat::Ifc => Err(ExportError::Format(
                "IFC собирается из модели, а не из листов".into(),
            )),
            DrawingFormat::Pdf => self.pdf(),
            DrawingFormat::Xlsx => xlsx::write(&self.worksheets),
            DrawingFormat::Svg => per_sheet("svg", &|s| {
                svg::write(
                    s,
                    SvgOptions {
                        text_as_paths: false,
                        embed_font: true,
                    },
                )
                .into_bytes()
            }),
            DrawingFormat::Dxf => per_sheet("dxf", &op_export::dxf::write),
            DrawingFormat::Bundle => op_export::zip(&[
                (self.file_name(DrawingFormat::Pdf), self.pdf()?),
                (
                    self.file_name(DrawingFormat::Xlsx),
                    xlsx::write(&self.worksheets)?,
                ),
            ]),
        }
    }

    fn pdf(&self) -> Result<Vec<u8>, ExportError> {
        pdf::write(
            &self.sheets,
            &PdfInfo {
                title: self.name.clone(),
                author: self.author.clone(),
            },
        )
    }
}

/// Самый крупный стандартный масштаб, при котором план помещается (не крупнее 1:50).
fn pick_scale(needed: f64) -> u32 {
    SCALES
        .iter()
        .copied()
        .filter(|&s| s >= AUTO_LARGEST)
        .find(|&s| f64::from(s) >= needed * (1.0 - 1e-9))
        .unwrap_or(SCALES[SCALES.len() - 1])
}

/// Знаменатель масштаба, при котором план размера `extent` (мм) помещается в область листа.
fn needed_denominator(plan: (DVec2, DVec2), extent: DVec2) -> f64 {
    let w = plan.1.x - plan.0.x - 2.0 * PLAN_MARGIN;
    let h = plan.1.y - plan.0.y - CAPTION_BAND - 2.0 * PLAN_MARGIN;
    if w < 1.0 || h < 1.0 {
        return f64::INFINITY;
    }
    (extent.x / w).max(extent.y / h)
}

// ---------------------------------------------------------------- раскладка

/// Где на листе план и таблицы.
#[derive(Debug, Clone)]
struct Placement {
    /// Область плана вместе с полосой названия.
    plan: (DVec2, DVec2),
    /// Таблицы и их левые верхние углы.
    tables: Vec<(Table, DVec2)>,
    /// Таблицы, не поместившиеся на лист.
    overflow: Vec<Table>,
}

/// Раскладка листа: таблицы справа (столбцом над основной надписью) или под
/// планом — выбирается та, где план крупнее и меньше таблиц уходит на продолжение.
fn place(size: DVec2, tables: Vec<Table>, extent: DVec2) -> Placement {
    let (lo, hi) = frame::inner(size);
    let candidates = [
        place_column(lo, hi, tables.clone()),
        Some(place_stack(lo, hi, tables)),
    ];
    let score = |p: &Placement| -> f64 {
        let need = needed_denominator(p.plan, extent);
        if need.is_finite() { 1.0 / need } else { 0.0 }
    };
    candidates
        .into_iter()
        .flatten()
        .min_by(|a, b| {
            a.overflow
                .len()
                .cmp(&b.overflow.len())
                .then(score(b).total_cmp(&score(a)))
        })
        .unwrap_or(Placement {
            plan: (lo, hi),
            tables: Vec::new(),
            overflow: Vec::new(),
        })
}

fn place_column(lo: DVec2, hi: DVec2, tables: Vec<Table>) -> Option<Placement> {
    let x = hi.x - TITLE_WIDTH;
    if x - lo.x < MIN_PLAN_WIDTH {
        return None;
    }
    let budget = hi.y - lo.y - TITLE_HEIGHT - 2.0 * GAP;
    let (placed, overflow) = flow(tables, budget);
    let mut y = lo.y + GAP;
    let tables = placed
        .into_iter()
        .map(|t| {
            let at = DVec2::new(hi.x - t.width(), y);
            y += t.height() + GAP;
            (t, at)
        })
        .collect();
    Some(Placement {
        plan: (lo, DVec2::new(x, hi.y)),
        tables,
        overflow,
    })
}

fn place_stack(lo: DVec2, hi: DVec2, tables: Vec<Table>) -> Placement {
    let free = hi.y - lo.y - TITLE_HEIGHT;
    let (placed, overflow) = flow(tables, free * STACK_SHARE);
    let total: f64 = placed.iter().map(|t| t.height() + GAP).sum();
    let mut y = hi.y - TITLE_HEIGHT - total;
    let bottom = y;
    let tables = placed
        .into_iter()
        .map(|t| {
            let at = DVec2::new(hi.x - t.width(), y);
            y += t.height() + GAP;
            (t, at)
        })
        .collect();
    Placement {
        plan: (lo, DVec2::new(hi.x, bottom)),
        tables,
        overflow,
    }
}

/// Раскладывает таблицы сверху вниз в высоту `budget`; таблицу, которая не
/// помещается, делит по строкам. Возвращает поместившиеся и остальные.
fn flow(tables: Vec<Table>, budget: f64) -> (Vec<Table>, Vec<Table>) {
    let mut placed: Vec<Table> = Vec::new();
    let mut rest: Vec<Table> = Vec::new();
    let mut used = 0.0;
    for table in tables {
        if !rest.is_empty() {
            rest.push(table);
            continue;
        }
        let gap = if placed.is_empty() { 0.0 } else { GAP };
        if used + gap + table.height() <= budget {
            used += gap + table.height();
            placed.push(table);
            continue;
        }
        match split(&table, budget - used - gap) {
            Some((head, tail)) => {
                used += gap + head.height();
                placed.push(head);
                rest.push(tail);
            }
            None => rest.push(table),
        }
    }
    (placed, rest)
}

/// Делит таблицу по строкам так, чтобы первая часть уместилась в `height`.
/// В первой части — не меньше двух строк, итог — во второй.
fn split(table: &Table, height: f64) -> Option<(Table, Table)> {
    let mut head = Table {
        footer: None,
        rows: Vec::new(),
        ..table.clone()
    };
    let mut n = 0;
    while n < table.rows.len() {
        head.rows = table.rows[..=n].to_vec();
        if head.height() > height {
            break;
        }
        n += 1;
    }
    let n = n.min(table.rows.len().saturating_sub(1));
    if n < 2 {
        return None;
    }
    head.rows = table.rows[..n].to_vec();
    let tail = Table {
        title: format!("{} (продолжение)", table.title),
        columns: table.columns.clone(),
        rows: table.rows[n..].to_vec(),
        footer: table.footer.clone(),
    };
    Some((head, tail))
}

/// Листы продолжения: таблицы столбцами слева направо, в обход основной надписи.
fn continuation(size: DVec2, mut tables: Vec<Table>) -> Vec<Vec<(Table, DVec2)>> {
    let (lo, hi) = frame::inner(size);
    let mut pages = Vec::new();
    while !tables.is_empty() {
        let mut page = Vec::new();
        let mut x = lo.x + GAP;
        while !tables.is_empty() && x + TITLE_WIDTH <= hi.x + 1e-9 {
            let under_title = x + TITLE_WIDTH > hi.x - TITLE_WIDTH;
            let bottom = if under_title {
                hi.y - TITLE_HEIGHT
            } else {
                hi.y
            };
            let (placed, rest) = flow(std::mem::take(&mut tables), bottom - lo.y - 2.0 * GAP);
            tables = rest;
            let mut y = lo.y + GAP;
            for t in placed {
                let at = DVec2::new(x, y);
                y += t.height() + GAP;
                page.push((t, at));
            }
            x += TITLE_WIDTH + GAP;
        }
        if page.is_empty() {
            break;
        }
        pages.push(page);
    }
    pages
}

// ---------------------------------------------------------------- модель

/// Обмер и вариант со своими производными данными.
struct Views<'a> {
    doc: &'a Document,
    survey: Document,
    survey_derived: Derived,
    variant: Option<VariantView>,
}

struct VariantView {
    id: String,
    doc: Document,
    derived: Derived,
    /// Демонтируемые элементы обмера.
    removed: HashSet<String>,
}

/// Строка ведомости с маркой на плане.
#[derive(Debug, Clone)]
struct Row {
    mark: String,
    name: String,
    thickness: f64,
    /// Длина (ширина проёма), м.
    length: f64,
    /// Высота, м.
    height: f64,
    /// Площадь, м².
    area: f64,
    /// Точка на грани конструкции, у которой ставится марка.
    at: DVec2,
    /// Направление от грани наружу.
    out: DVec2,
    /// Материал возводимой перегородки.
    material: Option<WallMaterial>,
    /// Закладка проёма.
    infill: bool,
}

/// Помещение в экспликации.
#[derive(Debug, Clone)]
struct RoomRow {
    id: String,
    name: String,
    area: Option<f64>,
    perimeter: Option<f64>,
    height: f64,
    /// Ширина помещения по габариту, мм (для подписи).
    width: Option<f64>,
}

fn material_name(m: Option<WallMaterial>) -> &'static str {
    match m {
        Some(WallMaterial::AeratedConcrete) => "Газобетон",
        Some(WallMaterial::Gypsum) => "ГКЛ",
        Some(WallMaterial::Brick) => "Кирпич",
        None => "Материал не указан",
    }
}

/// Материал в середине фразы: «перегородка: газобетон».
fn material_phrase(m: Option<WallMaterial>) -> &'static str {
    match m {
        Some(WallMaterial::AeratedConcrete) => "газобетон",
        Some(WallMaterial::Gypsum) => "ГКЛ",
        Some(WallMaterial::Brick) => "кирпич",
        None => "материал не указан",
    }
}

fn opening_name(kind: OpeningKind) -> &'static str {
    match kind {
        OpeningKind::Door => "дверной",
        OpeningKind::Window => "оконный",
        OpeningKind::Arch => "арочный",
    }
}

impl<'a> Views<'a> {
    fn new(doc: &'a Document, variant: Option<&str>, level: Option<&str>) -> Self {
        let survey = op_model::view::project_level(doc, None, level);
        let survey_derived = Derived::new(&survey);
        let variant = variant.and_then(|id| doc.variant(id)).map(|v| {
            let projected = op_model::view::project_level(doc, Some(&v.id), level);
            VariantView {
                id: v.id.clone(),
                derived: Derived::new(&projected),
                doc: projected,
                removed: demolished(doc, &v.id),
            }
        });
        Self {
            doc,
            survey,
            survey_derived,
            variant,
        }
    }

    fn height(&self, h: Option<f64>) -> f64 {
        h.unwrap_or(self.doc.default_ceiling_height) / 1000.0
    }

    /// Габарит плана (мм) по обмеру и варианту.
    fn extent(&self) -> Option<(DVec2, DVec2)> {
        let mut lo = DVec2::splat(f64::INFINITY);
        let mut hi = DVec2::splat(f64::NEG_INFINITY);
        let mut add = |p: DVec2| {
            lo = lo.min(p);
            hi = hi.max(p);
        };
        let views = std::iter::once((&self.survey, &self.survey_derived))
            .chain(self.variant.as_ref().map(|v| (&v.doc, &v.derived)));
        for (doc, derived) in views {
            let shown = |layer: &str| derived.visible.contains(layer);
            for w in doc.walls.iter().filter(|w| shown(&w.layer)) {
                for p in derived.wall_outlines.get(&w.id).into_iter().flatten() {
                    add(*p);
                }
            }
            for r in doc.rooms.iter().filter(|r| shown(&r.layer)) {
                add(DVec2::new(r.x, r.y));
            }
            for l in doc.leaders.iter().filter(|l| shown(&l.layer)) {
                add(DVec2::new(l.x, l.y));
                add(DVec2::new(l.text_x, l.text_y));
            }
            for d in doc.dimensions.iter().filter(|d| shown(&d.layer)) {
                if let Some(g) = geometry::dimension(derived, d) {
                    for p in [g.a, g.b, g.qa, g.qb] {
                        add(p);
                    }
                }
            }
        }
        lo.x.is_finite().then_some((lo, hi))
    }

    /// Помещения вида: метки в одной области (снесли перегородку) — одно помещение.
    fn rooms(
        doc: &Document,
        derived: &Derived,
        height: impl Fn(Option<f64>) -> f64,
    ) -> Vec<RoomRow> {
        let mut used = HashSet::new();
        doc.rooms
            .iter()
            .filter(|r| derived.visible.contains(&r.layer))
            .filter(|r| {
                derived
                    .room_regions
                    .get(&r.id)
                    .is_none_or(|&i| used.insert(i))
            })
            .map(|r| {
                let region = derived
                    .room_regions
                    .get(&r.id)
                    .map(|&i| &derived.regions[i]);
                RoomRow {
                    id: r.id.clone(),
                    name: r.name.clone(),
                    area: region.map(|g| op_geom::mm2_to_m2(g.area)),
                    perimeter: region.map(|g| g.perimeter / 1000.0),
                    height: height(r.ceiling_height),
                    width: region.map(|g| {
                        let (lo, hi) = g
                            .contour
                            .iter()
                            .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), p| {
                                (lo.min(p.x), hi.max(p.x))
                            });
                        hi - lo
                    }),
                }
            })
            .collect()
    }

    fn survey_rooms(&self) -> Vec<RoomRow> {
        Self::rooms(&self.survey, &self.survey_derived, |h| self.height(h))
    }

    fn variant_rooms(&self) -> Vec<RoomRow> {
        self.variant.as_ref().map_or_else(Vec::new, |v| {
            Self::rooms(&v.doc, &v.derived, |h| self.height(h))
        })
    }

    /// Демонтаж: стены обмера и проёмы, прорезаемые в стенах обмера.
    fn demolition_rows(&self) -> Vec<Row> {
        let Some(v) = &self.variant else {
            return Vec::new();
        };
        let (doc, derived) = (&self.survey, &self.survey_derived);
        let mut rows = Vec::new();
        for w in doc.walls.iter().filter(|w| v.removed.contains(&w.id)) {
            let Some((a, b)) = derived.wall_axis(w) else {
                continue;
            };
            let length = a.distance(b) / 1000.0;
            let height = self.height(w.height);
            rows.push(Row {
                mark: String::new(),
                name: match w.material {
                    Some(m) => format!("Стена ({})", material_phrase(Some(m))),
                    None => "Стена".to_owned(),
                },
                thickness: w.thickness,
                length,
                height,
                area: length * height,
                at: face_point(a, b, w).0,
                out: face_point(a, b, w).1,
                material: None,
                infill: false,
            });
        }
        // Новые проёмы в стенах обмера — вырез части стены.
        for o in v.doc.openings.iter().filter(|o| o.variant.is_some()) {
            let Some(wall) = v.doc.wall(&o.wall).filter(|w| w.variant.is_none()) else {
                continue;
            };
            let Some(g) = geometry::opening(&v.doc, &v.derived, o) else {
                continue;
            };
            let (width, height) = (o.width / 1000.0, o.height / 1000.0);
            rows.push(Row {
                mark: String::new(),
                name: format!("Проём {} в стене", opening_name(o.kind)),
                thickness: wall.thickness,
                length: width,
                height,
                area: width * height,
                at: g.start.midpoint(g.end) + g.normal * g.left,
                out: g.normal,
                material: None,
                infill: false,
            });
        }
        for (i, row) in rows.iter_mut().enumerate() {
            row.mark = format!("Д{}", i + 1);
        }
        rows
    }

    /// Монтаж: новые стены варианта и закладка проёмов обмера.
    fn construction_rows(&self) -> Vec<Row> {
        let Some(v) = &self.variant else {
            return Vec::new();
        };
        let mut rows = Vec::new();
        for w in v.doc.walls.iter().filter(|w| w.variant.is_some()) {
            let Some((a, b)) = v.derived.wall_axis(w) else {
                continue;
            };
            let length = a.distance(b) / 1000.0;
            let height = self.height(w.height);
            let openings: f64 = v
                .doc
                .openings
                .iter()
                .filter(|o| o.wall == w.id)
                .map(|o| o.width * o.height / 1e6)
                .sum();
            rows.push(Row {
                mark: String::new(),
                name: format!("Перегородка: {}", material_phrase(w.material)),
                thickness: w.thickness,
                length,
                height,
                area: (length * height - openings).max(0.0),
                at: face_point(a, b, w).0,
                out: face_point(a, b, w).1,
                material: w.material,
                infill: false,
            });
        }
        let walls_removed: HashSet<&str> = self
            .survey
            .walls
            .iter()
            .filter(|w| v.removed.contains(&w.id))
            .map(|w| w.id.as_str())
            .collect();
        for o in self
            .survey
            .openings
            .iter()
            .filter(|o| v.removed.contains(&o.id) && !walls_removed.contains(o.wall.as_str()))
        {
            let Some(wall) = self.survey.wall(&o.wall) else {
                continue;
            };
            let Some(g) = geometry::opening(&self.survey, &self.survey_derived, o) else {
                continue;
            };
            let (width, height) = (o.width / 1000.0, o.height / 1000.0);
            rows.push(Row {
                mark: String::new(),
                name: format!("Закладка проёма ({})", opening_name(o.kind)),
                thickness: wall.thickness,
                length: width,
                height,
                area: width * height,
                at: g.start.midpoint(g.end) + g.normal * g.left,
                out: g.normal,
                material: wall.material,
                infill: true,
            });
        }
        for (i, row) in rows.iter_mut().enumerate() {
            row.mark = format!("П{}", i + 1);
        }
        rows
    }

    /// Таблицы листа (со легендами).
    fn tables(&self, kind: Kind) -> Vec<Table> {
        let mut out = Vec::new();
        match kind {
            Kind::Survey => {
                out.extend(rooms_table("Экспликация помещений", &self.survey_rooms()));
            }
            Kind::Demolition => {
                out.push(demolition_table(&self.demolition_rows()));
                out.push(legend(&[
                    (existing_swatch(), "Существующие стены и перегородки"),
                    (demolition_swatch(), "Демонтируемые конструкции"),
                ]));
            }
            Kind::Construction => {
                let rows = self.construction_rows();
                out.push(construction_table(&rows));
                out.push(materials_table(&rows));
                let mut items = vec![(
                    existing_swatch(),
                    "Существующие стены и перегородки".to_owned(),
                )];
                let mut seen = Vec::new();
                for r in rows.iter().filter(|r| !r.infill) {
                    if !seen.contains(&r.material) {
                        seen.push(r.material);
                        items.push((
                            material_swatch(r.material),
                            format!("Возводимые перегородки: {}", material_phrase(r.material)),
                        ));
                    }
                }
                if rows.iter().any(|r| r.infill) {
                    items.push((infill_swatch(), "Закладка проёмов".to_owned()));
                }
                let items: Vec<(Vec<Mark>, &str)> =
                    items.iter().map(|(s, n)| (s.clone(), n.as_str())).collect();
                out.push(legend(&items));
            }
            Kind::Rooms => {
                out.extend(rooms_table(
                    "Экспликация помещений после перепланировки",
                    &self.variant_rooms(),
                ));
            }
            Kind::Furniture => out.push(furniture_table(&self.furniture_rows())),
            Kind::Sockets => out.push(self.outlets_table()),
            Kind::Lighting => {
                out.push(self.lights_table());
                out.push(illuminance_table(&self.electrical().rooms));
            }
            Kind::Switches => {
                out.push(self.switches_table());
                out.extend(self.links_table());
                out.extend(self.scenes_table());
            }
            Kind::Wiring => {
                let report = self.electrical();
                out.push(circuits_table(self.final_view().0, &report));
                out.push(panel_table(&report.panel));
            }
            Kind::Water | Kind::Sewer => {
                let water = kind == Kind::Water;
                out.push(self.plumbing_points_table(water));
                if !water {
                    out.extend(self.drain_pipes_table());
                }
                out.extend(self.bill_table(water));
                out.extend(self.plumbing_notes(water));
            }
            Kind::Floors => {
                out.push(self.finishes_table(Some(op_model::Surface::Floor)));
                out.extend(self.totals_table());
            }
            Kind::Ceilings => out.push(self.finishes_table(Some(op_model::Surface::Ceiling))),
            Kind::Climate => {
                out.extend(self.heating_table());
                out.push(self.air_table());
            }
        }
        out
    }

    /// Расчёт электрики итогового вида.
    fn electrical(&self) -> crate::protocol::ElectricalReport {
        let (doc, derived) = self.final_view();
        electric::report(doc, derived)
    }

    /// Видимые электроточки итогового вида, отобранные `keep`.
    fn devices(&self, keep: impl Fn(op_model::DeviceKind) -> bool) -> Vec<&op_model::Device> {
        let (doc, derived) = self.final_view();
        doc.devices
            .iter()
            .filter(|d| derived.visible.contains(&d.layer) && keep(d.kind))
            .collect()
    }

    /// Марки светильников «С1…» и выключателей «В1…» в порядке документа.
    fn device_marks(&self) -> HashMap<String, String> {
        let mut marks = HashMap::new();
        let (mut lights, mut switches) = (0, 0);
        for d in self.devices(|_| true) {
            if d.kind.is_light() {
                lights += 1;
                marks.insert(d.id.clone(), format!("С{lights}"));
            } else if electric::is_switch(d.kind) {
                switches += 1;
                marks.insert(d.id.clone(), format!("В{switches}"));
            }
        }
        marks
    }

    fn room_name(&self, d: &op_model::Device) -> String {
        let (doc, derived) = self.final_view();
        shapes::region_at(&derived.regions, DVec2::new(d.x, d.y))
            .and_then(|i| {
                doc.rooms
                    .iter()
                    .find(|r| derived.room_regions.get(&r.id) == Some(&i))
            })
            .map_or_else(|| "—".to_owned(), |r| r.name.clone())
    }

    fn outlets_table(&self) -> Table {
        let mut rows: Vec<(String, String, f64, String, usize)> = Vec::new();
        for d in self.devices(electric::is_outlet) {
            let key = (
                self.room_name(d),
                electric::kind_name(d.kind).to_owned(),
                d.height.round(),
                d.label.clone(),
            );
            match rows
                .iter_mut()
                .find(|r| r.0 == key.0 && r.1 == key.1 && (r.2 - key.2).abs() < 0.5 && r.3 == key.3)
            {
                Some(r) => r.4 += 1,
                None => rows.push((key.0, key.1, key.2, key.3, 1)),
            }
        }
        let count: usize = rows.iter().map(|r| r.4).sum();
        Table {
            title: "Ведомость розеток и выводов".into(),
            columns: vec![
                Column::new("Помещение", 45.0, Align::Left),
                Column::new("Тип", 58.0, Align::Left),
                Column::new("H, мм", 18.0, Align::Right),
                Column::new("Назначение", 46.0, Align::Left),
                Column::new("Кол.", 18.0, Align::Right),
            ],
            rows: rows
                .into_iter()
                .map(|r| {
                    vec![
                        Cell::text(r.0),
                        Cell::text(r.1),
                        Cell::number(r.2, 0),
                        Cell::text(r.3),
                        Cell::text(r.4.to_string()),
                    ]
                })
                .collect(),
            footer: Some(vec![
                Cell::text("Итого"),
                Cell::text(""),
                Cell::text(""),
                Cell::text(""),
                Cell::text(count.to_string()),
            ]),
        }
    }

    fn lights_table(&self) -> Table {
        let mut rows: Vec<(String, f64, f64, usize)> = Vec::new();
        for d in self.devices(op_model::DeviceKind::is_light) {
            let name = electric::kind_name(d.kind).to_owned();
            match rows
                .iter_mut()
                .find(|r| r.0 == name && (r.1 - d.flux).abs() < 0.5 && (r.2 - d.power).abs() < 0.05)
            {
                Some(r) => r.3 += 1,
                None => rows.push((name, d.flux, d.power, 1)),
            }
        }
        #[allow(clippy::cast_precision_loss)] // светильников — единицы и десятки
        let power: f64 = rows.iter().map(|r| r.2 * r.3 as f64).sum();
        Table {
            title: "Светильники".into(),
            columns: vec![
                Column::new("Тип", 95.0, Align::Left),
                Column::new("Φ, лм", 30.0, Align::Right),
                Column::new("P, Вт", 30.0, Align::Right),
                Column::new("Кол.", 30.0, Align::Right),
            ],
            rows: rows
                .iter()
                .map(|r| {
                    vec![
                        Cell::text(r.0.clone()),
                        Cell::number(r.1, 0),
                        Cell::number(r.2, 0),
                        Cell::text(r.3.to_string()),
                    ]
                })
                .collect(),
            footer: Some(vec![
                Cell::text("Итого, Вт"),
                Cell::text(""),
                Cell::number(power, 0),
                Cell::text(rows.iter().map(|r| r.3).sum::<usize>().to_string()),
            ]),
        }
    }

    fn switches_table(&self) -> Table {
        let mut rows: Vec<(String, f64, usize)> = Vec::new();
        for d in self.devices(electric::is_switch) {
            let name = electric::kind_name(d.kind).to_owned();
            match rows
                .iter_mut()
                .find(|r| r.0 == name && (r.1 - d.height).abs() < 0.5)
            {
                Some(r) => r.2 += 1,
                None => rows.push((name, d.height.round(), 1)),
            }
        }
        Table {
            title: "Выключатели".into(),
            columns: vec![
                Column::new("Тип", 115.0, Align::Left),
                Column::new("H, мм", 35.0, Align::Right),
                Column::new("Кол.", 35.0, Align::Right),
            ],
            rows: rows
                .into_iter()
                .map(|r| {
                    vec![
                        Cell::text(r.0),
                        Cell::number(r.1, 0),
                        Cell::text(r.2.to_string()),
                    ]
                })
                .collect(),
            footer: None,
        }
    }

    /// Какие светильники включает каждая клавиша.
    fn links_table(&self) -> Option<Table> {
        let marks = self.device_marks();
        let mut rows = Vec::new();
        for d in self.devices(electric::is_switch) {
            for (k, key) in d.keys.iter().enumerate() {
                let lights: Vec<&str> = key
                    .iter()
                    .filter_map(|l| marks.get(l).map(String::as_str))
                    .collect();
                if lights.is_empty() {
                    continue;
                }
                rows.push(vec![
                    Cell::text(marks.get(&d.id).cloned().unwrap_or_default()),
                    Cell::text((k + 1).to_string()),
                    Cell::text(lights.join(", ")),
                ]);
            }
        }
        (!rows.is_empty()).then(|| Table {
            title: "Управление светом".into(),
            columns: vec![
                Column::new("Выключатель", 35.0, Align::Center),
                Column::new("Клавиша", 30.0, Align::Center),
                Column::new("Светильники", 120.0, Align::Left),
            ],
            rows,
            footer: None,
        })
    }

    fn scenes_table(&self) -> Option<Table> {
        let (doc, _) = self.final_view();
        let marks = self.device_marks();
        (!doc.scenes.is_empty()).then(|| Table {
            title: "Сценарии освещения".into(),
            columns: vec![
                Column::new("Сценарий", 65.0, Align::Left),
                Column::new("Светильники", 120.0, Align::Left),
            ],
            rows: doc
                .scenes
                .iter()
                .map(|s| {
                    let lights: Vec<&str> = s
                        .lights
                        .iter()
                        .filter_map(|l| marks.get(l).map(String::as_str))
                        .collect();
                    vec![Cell::text(s.name.clone()), Cell::text(lights.join(", "))]
                })
                .collect(),
            footer: None,
        })
    }

    /// Итоговый вид: вариант, если он выбран, иначе обмер.
    fn final_view(&self) -> (&Document, &Derived) {
        match &self.variant {
            Some(v) => (&v.doc, &v.derived),
            None => (&self.survey, &self.survey_derived),
        }
    }

    /// Спецификация мебели итогового вида: одинаковые предметы — одной позицией.
    fn furniture_rows(&self) -> Vec<FurnitureRow> {
        let (doc, derived) = self.final_view();
        let mut rows: Vec<FurnitureRow> = Vec::new();
        for f in doc
            .furniture
            .iter()
            .filter(|f| derived.visible.contains(&f.layer))
        {
            let size = [f.width, f.depth, f.height].map(f64::round);
            let same = |r: &FurnitureRow| {
                r.name == f.name && r.size.iter().zip(size).all(|(a, b)| (a - b).abs() < 0.5)
            };
            match rows.iter_mut().find(|r| same(r)) {
                Some(r) => r.ids.push(f.id.clone()),
                None => rows.push(FurnitureRow {
                    name: f.name.clone(),
                    size,
                    ids: vec![f.id.clone()],
                }),
            }
        }
        rows
    }

    /// Ведомости для XLSX.
    fn worksheets(&self) -> Vec<Worksheet> {
        let mut out = Vec::new();
        let survey = self.survey_rooms();
        out.push(Worksheet {
            name: "Обмер".into(),
            tables: rooms_table("Экспликация помещений (обмер)", &survey)
                .into_iter()
                .collect(),
        });
        if let Some(v) = &self.variant {
            let demolition = self.demolition_rows();
            if !demolition.is_empty() {
                out.push(Worksheet {
                    name: "Демонтаж".into(),
                    tables: vec![demolition_table(&demolition)],
                });
            }
            let construction = self.construction_rows();
            if !construction.is_empty() {
                out.push(Worksheet {
                    name: "Монтаж".into(),
                    tables: vec![
                        construction_table(&construction),
                        materials_table(&construction),
                    ],
                });
            }
            let name = view_name(self.doc, Some(&v.id));
            out.push(Worksheet {
                name: name.clone(),
                tables: rooms_table(
                    &format!("Экспликация помещений ({name})"),
                    &self.variant_rooms(),
                )
                .into_iter()
                .collect(),
            });
        }
        let furniture = self.furniture_rows();
        if !furniture.is_empty() {
            out.push(Worksheet {
                name: "Мебель".into(),
                tables: vec![furniture_table(&furniture)],
            });
        }
        if !self.devices(|_| true).is_empty() {
            let report = self.electrical();
            let mut tables = Vec::new();
            if !self.devices(electric::is_outlet).is_empty() {
                tables.push(self.outlets_table());
            }
            if !self.devices(op_model::DeviceKind::is_light).is_empty() {
                tables.push(self.lights_table());
                tables.push(illuminance_table(&report.rooms));
            }
            if !self.final_view().0.circuits.is_empty() {
                tables.push(circuits_table(self.final_view().0, &report));
                tables.push(panel_table(&report.panel));
            }
            out.push(Worksheet {
                name: "Электрика".into(),
                tables,
            });
        }
        let mut tables = Vec::new();
        for water in [true, false] {
            if self.has_plumbing(water) {
                tables.push(self.plumbing_points_table(water));
                if !water {
                    tables.extend(self.drain_pipes_table());
                }
                tables.extend(self.bill_table(water));
            }
        }
        if !tables.is_empty() {
            out.push(Worksheet {
                name: "Сантехника".into(),
                tables,
            });
        }
        if self.has_climate() {
            out.push(Worksheet {
                name: "Климат".into(),
                tables: self
                    .heating_table()
                    .into_iter()
                    .chain(std::iter::once(self.air_table()))
                    .collect(),
            });
        }
        if !self.final_view().0.finishes.is_empty() {
            out.push(Worksheet {
                name: "Отделка".into(),
                tables: std::iter::once(self.finishes_table(None))
                    .chain(self.totals_table())
                    .collect(),
            });
        }
        out
    }

    /// План листа.
    fn draw(&self, kind: Kind, plot: &mut Plot<'_>) {
        match (kind, &self.variant) {
            (Kind::Water | Kind::Sewer, _) => self.draw_plumbing(kind == Kind::Water, plot),
            (Kind::Floors, _) => self.draw_finishes(op_model::Surface::Floor, plot),
            (Kind::Ceilings, _) => self.draw_finishes(op_model::Surface::Ceiling, plot),
            (Kind::Climate, _) => self.draw_climate(plot),
            (Kind::Sockets | Kind::Lighting | Kind::Switches | Kind::Wiring, _) => {
                let (doc, derived) = self.final_view();
                let rooms = match &self.variant {
                    Some(_) => self.variant_rooms(),
                    None => self.survey_rooms(),
                };
                plot.walls(LAYER_WALLS, &walls_where(doc, derived, |_| true));
                plot.openings(doc, derived, |_| true);
                plot.room_labels(doc, &rooms, false);
                let marks = self.device_marks();
                match kind {
                    Kind::Sockets => {
                        plot.devices(&self.devices(electric::is_outlet), |d| {
                            Some(geometry::format_length(d.height))
                        });
                    }
                    Kind::Lighting => {
                        plot.devices(&self.devices(op_model::DeviceKind::is_light), |d| {
                            marks.get(&d.id).cloned()
                        });
                    }
                    Kind::Switches => {
                        let switches = self.devices(electric::is_switch);
                        let lights = self.devices(op_model::DeviceKind::is_light);
                        plot.switch_links(&switches, &lights);
                        plot.devices(&lights, |d| marks.get(&d.id).cloned());
                        plot.devices(&switches, |d| marks.get(&d.id).cloned());
                    }
                    _ => {
                        let all = self.devices(|_| true);
                        for c in &doc.circuits {
                            let (path, _) = electric::route(doc, &c.id);
                            plot.route(&path, &c.name);
                        }
                        plot.devices(&all, |_| None);
                    }
                }
            }
            (Kind::Furniture, _) => {
                let (doc, derived) = self.final_view();
                let rooms = match &self.variant {
                    Some(_) => self.variant_rooms(),
                    None => self.survey_rooms(),
                };
                plot.walls(LAYER_WALLS, &walls_where(doc, derived, |_| true));
                plot.openings(doc, derived, |_| true);
                plot.room_labels(doc, &rooms, false);
                plot.furniture(doc, derived, &self.furniture_rows());
            }
            (Kind::Survey, _) | (_, None) => {
                let (doc, derived) = (&self.survey, &self.survey_derived);
                plot.walls(LAYER_WALLS, &walls_where(doc, derived, |_| true));
                plot.openings(doc, derived, |_| true);
                plot.room_labels(doc, &self.survey_rooms(), true);
                plot.dimensions(doc, derived);
                plot.leaders(doc, derived);
            }
            (Kind::Demolition, Some(v)) => {
                let (doc, derived) = (&self.survey, &self.survey_derived);
                plot.walls(
                    LAYER_WALLS,
                    &walls_where(doc, derived, |w| !v.removed.contains(&w.id)),
                );
                let gone = walls_where(doc, derived, |w| v.removed.contains(&w.id));
                plot.demolished(&gone.iter().flat_map(rings).collect::<Vec<_>>());
                plot.openings(doc, derived, |o| !v.removed.contains(&o.wall));
                // Новые проёмы в стенах обмера: вырезаемая часть стены.
                for o in v.doc.openings.iter().filter(|o| {
                    o.variant.is_some() && v.doc.wall(&o.wall).is_some_and(|w| w.variant.is_none())
                }) {
                    if let Some(g) = geometry::opening(&v.doc, &v.derived, o) {
                        plot.demolished(&[g.rect]);
                    }
                }
                plot.room_labels(doc, &self.survey_rooms(), false);
                plot.dimensions(doc, derived);
                plot.marks(LAYER_DEMOLITION, &self.demolition_rows(), DEMOLISH);
            }
            (Kind::Construction, Some(v)) => {
                let (doc, derived) = (&v.doc, &v.derived);
                plot.walls(
                    LAYER_WALLS,
                    &walls_where(doc, derived, |w| w.variant.is_none()),
                );
                let mut materials: Vec<Option<WallMaterial>> = doc
                    .walls
                    .iter()
                    .filter(|w| w.variant.is_some())
                    .map(|w| w.material)
                    .collect();
                materials.sort_by_key(|m| m.map(|m| m as u8));
                materials.dedup();
                for m in materials {
                    let shapes =
                        walls_where(doc, derived, |w| w.variant.is_some() && w.material == m);
                    plot.material(&shapes.iter().flat_map(rings).collect::<Vec<_>>(), Some(m));
                }
                let rows = self.construction_rows();
                // Закладка проёмов обмера.
                for o in self
                    .survey
                    .openings
                    .iter()
                    .filter(|o| v.removed.contains(&o.id) && !v.removed.contains(&o.wall))
                {
                    if let Some(g) = geometry::opening(&self.survey, &self.survey_derived, o) {
                        plot.material(&[g.rect], None);
                    }
                }
                plot.openings(doc, derived, |_| true);
                plot.room_labels(doc, &self.variant_rooms(), false);
                plot.dimensions(doc, derived);
                plot.marks(LAYER_CONSTRUCTION, &rows, Rgb::BLACK);
            }
            (Kind::Rooms, Some(v)) => {
                let (doc, derived) = (&v.doc, &v.derived);
                plot.walls(LAYER_WALLS, &walls_where(doc, derived, |_| true));
                plot.openings(doc, derived, |_| true);
                plot.room_labels(doc, &self.variant_rooms(), true);
                plot.leaders(doc, derived);
            }
        }
    }
}

/// Точка на левой грани стены для марки и направление от грани наружу. Марка
/// стоит на трети длины, а не в середине, где обычно подписи помещений.
fn face_point(a: DVec2, b: DVec2, w: &Wall) -> (DVec2, DVec2) {
    let normal = op_geom::left_normal((b - a).normalize_or_zero());
    let (left, _) = w.sides();
    (a.lerp(b, 0.3) + normal * left, normal)
}

/// Стены видимых слоёв, отобранные `keep`: объединение минус проёмы.
fn walls_where(doc: &Document, derived: &Derived, keep: impl Fn(&Wall) -> bool) -> Vec<Shape> {
    let outlines: Vec<Vec<DVec2>> = doc
        .walls
        .iter()
        .filter(|w| derived.visible.contains(&w.layer) && keep(w))
        .filter_map(|w| derived.wall_outlines.get(&w.id))
        .filter(|o| !o.is_empty())
        .cloned()
        .collect();
    let cuts: Vec<Vec<DVec2>> = doc
        .openings
        .iter()
        .filter_map(|o| geometry::opening(doc, derived, o))
        .map(|g| g.rect)
        .collect();
    shapes::difference(&outlines, &cuts)
}

fn rings(shape: &Shape) -> Vec<Vec<DVec2>> {
    std::iter::once(shape.outer.clone())
        .chain(shape.holes.iter().cloned())
        .collect()
}

// ---------------------------------------------------------------- таблицы

fn number_or_dash(value: Option<f64>, decimals: u8) -> Cell {
    value.map_or_else(|| Cell::text("—"), |v| Cell::number(v, decimals))
}

fn rooms_table(title: &str, rooms: &[RoomRow]) -> Option<Table> {
    if rooms.is_empty() {
        return None;
    }
    let total: f64 = rooms.iter().filter_map(|r| r.area).sum();
    Some(Table {
        title: title.to_owned(),
        columns: vec![
            Column::new("№", 12.0, Align::Center),
            Column::new("Наименование", 83.0, Align::Left),
            Column::new("Площадь, м²", 30.0, Align::Right),
            Column::new("Периметр, м", 30.0, Align::Right),
            Column::new("Высота, м", 30.0, Align::Right),
        ],
        rows: rooms
            .iter()
            .enumerate()
            .map(|(i, r)| {
                vec![
                    Cell::text((i + 1).to_string()),
                    Cell::text(r.name.clone()),
                    number_or_dash(r.area, 2),
                    number_or_dash(r.perimeter, 2),
                    Cell::number(r.height, 2),
                ]
            })
            .collect(),
        footer: Some(vec![
            Cell::text(""),
            Cell::text("Итого"),
            Cell::number(total, 2),
            Cell::text(""),
            Cell::text(""),
        ]),
    })
}

fn demolition_table(rows: &[Row]) -> Table {
    let area: f64 = rows.iter().map(|r| r.area).sum();
    let volume: f64 = rows.iter().map(|r| r.area * r.thickness / 1000.0).sum();
    Table {
        title: "Ведомость демонтажа".into(),
        columns: vec![
            Column::new("Марка", 14.0, Align::Center),
            Column::new("Конструкция", 59.0, Align::Left),
            Column::new("Толщина, мм", 22.0, Align::Right),
            Column::new("Длина, м", 22.0, Align::Right),
            Column::new("Высота, м", 22.0, Align::Right),
            Column::new("Площадь, м²", 23.0, Align::Right),
            Column::new("Объём, м³", 23.0, Align::Right),
        ],
        rows: rows
            .iter()
            .map(|r| {
                vec![
                    Cell::text(r.mark.clone()),
                    Cell::text(r.name.clone()),
                    Cell::number(r.thickness, 0),
                    Cell::number(r.length, 2),
                    Cell::number(r.height, 2),
                    Cell::number(r.area, 2),
                    Cell::number(r.area * r.thickness / 1000.0, 2),
                ]
            })
            .collect(),
        footer: Some(vec![
            Cell::text(""),
            Cell::text("Итого"),
            Cell::text(""),
            Cell::text(""),
            Cell::text(""),
            Cell::number(area, 2),
            Cell::number(volume, 2),
        ]),
    }
}

fn construction_table(rows: &[Row]) -> Table {
    let area: f64 = rows.iter().map(|r| r.area).sum();
    Table {
        title: "Ведомость перегородок и закладки проёмов".into(),
        columns: vec![
            Column::new("Марка", 14.0, Align::Center),
            Column::new("Конструкция", 82.0, Align::Left),
            Column::new("Толщина, мм", 22.0, Align::Right),
            Column::new("Длина, м", 22.0, Align::Right),
            Column::new("Высота, м", 22.0, Align::Right),
            Column::new("Площадь, м²", 23.0, Align::Right),
        ],
        rows: rows
            .iter()
            .map(|r| {
                vec![
                    Cell::text(r.mark.clone()),
                    Cell::text(r.name.clone()),
                    Cell::number(r.thickness, 0),
                    Cell::number(r.length, 2),
                    Cell::number(r.height, 2),
                    Cell::number(r.area, 2),
                ]
            })
            .collect(),
        footer: Some(vec![
            Cell::text(""),
            Cell::text("Итого"),
            Cell::text(""),
            Cell::text(""),
            Cell::text(""),
            Cell::number(area, 2),
        ]),
    }
}

fn illuminance_table(rooms: &[crate::protocol::RoomLighting]) -> Table {
    Table {
        title: "Освещённость помещений".into(),
        columns: vec![
            Column::new("Помещение", 55.0, Align::Left),
            Column::new("S, м²", 22.0, Align::Right),
            Column::new("Φ, лм", 24.0, Align::Right),
            Column::new("E, лк", 22.0, Align::Right),
            Column::new("Норма, лк", 24.0, Align::Right),
            Column::new("Вывод", 38.0, Align::Left),
        ],
        rows: rooms
            .iter()
            .map(|r| {
                vec![
                    Cell::text(r.name.clone()),
                    Cell::number(r.area, 2),
                    Cell::number(r.flux, 0),
                    Cell::number(r.lux, 0),
                    Cell::number(r.norm, 0),
                    Cell::text(if r.lux + 1e-9 >= r.norm {
                        "соответствует"
                    } else {
                        "ниже нормы"
                    }),
                ]
            })
            .collect(),
        footer: None,
    }
}

fn circuits_table(doc: &Document, report: &crate::protocol::ElectricalReport) -> Table {
    Table {
        title: "Группы щита".into(),
        columns: vec![
            Column::new("Гр.", 13.0, Align::Center),
            Column::new("Назначение", 43.0, Align::Left),
            Column::new("Аппарат", 30.0, Align::Left),
            Column::new("Кабель", 22.0, Align::Left),
            Column::new("L, м", 13.0, Align::Right),
            Column::new("Pр, кВт", 16.0, Align::Right),
            Column::new("I, А", 14.0, Align::Right),
            Column::new("ΔU, %", 14.0, Align::Right),
            Column::new("Кс", 10.0, Align::Right),
            Column::new("!", 10.0, Align::Center),
        ],
        rows: report
            .circuits
            .iter()
            .filter_map(|r| {
                let c = doc.circuits.iter().find(|c| c.id == r.id)?;
                let apparatus = format!(
                    "{} C{:.0}",
                    if c.rcd {
                        "Дифавт."
                    } else {
                        "Автомат"
                    },
                    c.breaker
                );
                let cores = if c.phases == 3 { 5 } else { 3 };
                Some(vec![
                    Cell::text(c.name.clone()),
                    Cell::text(c.purpose.clone()),
                    Cell::text(apparatus),
                    Cell::text(format!(
                        "{cores}×{}",
                        c.section.to_string().replace('.', ",")
                    )),
                    Cell::number(r.length, 1),
                    Cell::number(r.demand / 1000.0, 2),
                    Cell::number(r.current, 1),
                    Cell::number(r.drop, 1),
                    Cell::number(c.demand, 1),
                    Cell::text(if r.warnings.is_empty() { "" } else { "!" }),
                ])
            })
            .collect(),
        footer: None,
    }
}

fn panel_table(panel: &crate::protocol::PanelReport) -> Table {
    Table {
        title: "Состав щита".into(),
        columns: vec![
            Column::new("Аппарат", 135.0, Align::Left),
            Column::new("Модулей", 25.0, Align::Right),
            Column::new("Кол.", 25.0, Align::Right),
        ],
        rows: panel
            .items
            .iter()
            .map(|i| {
                vec![
                    Cell::text(i.name.clone()),
                    Cell::text(i.modules.to_string()),
                    Cell::text(i.count.to_string()),
                ]
            })
            .collect(),
        footer: Some(vec![
            Cell::text(format!("Итого; корпус на {} модулей", panel.enclosure)),
            Cell::text(panel.modules.to_string()),
            Cell::text(""),
        ]),
    }
}

/// Позиция спецификации мебели.
#[derive(Debug, Clone)]
struct FurnitureRow {
    name: String,
    /// Ширина, глубина, высота, мм.
    size: [f64; 3],
    /// Предметы этой позиции.
    ids: Vec<String>,
}

fn furniture_table(rows: &[FurnitureRow]) -> Table {
    let count: usize = rows.iter().map(|r| r.ids.len()).sum();
    Table {
        title: "Спецификация мебели и оборудования".into(),
        columns: vec![
            Column::new("Поз.", 14.0, Align::Center),
            Column::new("Наименование", 96.0, Align::Left),
            Column::new("Размеры (Ш × Г × В), мм", 55.0, Align::Center),
            Column::new("Кол.", 20.0, Align::Right),
        ],
        rows: rows
            .iter()
            .enumerate()
            .map(|(i, r)| {
                let [w, d, h] = r.size;
                vec![
                    Cell::text((i + 1).to_string()),
                    Cell::text(r.name.clone()),
                    Cell::text(format!(
                        "{} × {} × {}",
                        geometry::format_length(w),
                        geometry::format_length(d),
                        geometry::format_length(h)
                    )),
                    Cell::text(r.ids.len().to_string()),
                ]
            })
            .collect(),
        footer: Some(vec![
            Cell::text(""),
            Cell::text("Итого"),
            Cell::text(""),
            Cell::text(count.to_string()),
        ]),
    }
}

/// Итог по материалам: длина и площадь (площадь стен — за вычетом проёмов).
fn materials_table(rows: &[Row]) -> Table {
    let mut groups: Vec<(String, f64, f64)> = Vec::new();
    for r in rows {
        let name = if r.infill {
            "Закладка проёмов".to_owned()
        } else {
            material_name(r.material).to_owned()
        };
        match groups.iter_mut().find(|g| g.0 == name) {
            Some(g) => {
                g.1 += r.length;
                g.2 += r.area;
            }
            None => groups.push((name, r.length, r.area)),
        }
    }
    Table {
        title: "Итого по материалам".into(),
        columns: vec![
            Column::new("Материал", 125.0, Align::Left),
            Column::new("Длина, м", 30.0, Align::Right),
            Column::new("Площадь, м²", 30.0, Align::Right),
        ],
        rows: groups
            .into_iter()
            .map(|(name, length, area)| {
                vec![
                    Cell::text(name),
                    Cell::number(length, 2),
                    Cell::number(area, 2),
                ]
            })
            .collect(),
        footer: None,
    }
}

/// Ширина столбца образца в легенде, мм.
const SWATCH_COLUMN: f64 = 30.0;
/// Размер образца внутри ячейки, мм (ширина столбца минус поля, строка минус поля).
const SWATCH: DVec2 = DVec2::new(SWATCH_COLUMN - 6.0, 3.6);

fn legend(items: &[(Vec<Mark>, &str)]) -> Table {
    Table {
        title: "Условные обозначения".into(),
        columns: vec![
            Column::new("Обозначение", SWATCH_COLUMN, Align::Center),
            Column::new("Наименование", TITLE_WIDTH - SWATCH_COLUMN, Align::Left),
        ],
        rows: items
            .iter()
            .map(|(swatch, name)| vec![Cell::Swatch(swatch.clone()), Cell::text(*name)])
            .collect(),
        footer: None,
    }
}

/// Образец: примитивы в миллиметрах образца, переведённые в квадрат 1 × 1.
fn swatch(build: impl FnOnce(&mut Vec<Item>, &[Vec<DVec2>])) -> Vec<Mark> {
    let ring = op_export::sheet::rect_points(DVec2::ZERO, SWATCH);
    let mut items = Vec::new();
    build(&mut items, &[ring]);
    let unit = |p: DVec2| p / SWATCH;
    items
        .into_iter()
        .map(|item| match item.shape {
            Mark::Fill { rings, color } => Mark::Fill {
                rings: rings
                    .iter()
                    .map(|r| r.iter().map(|p| unit(*p)).collect())
                    .collect(),
                color,
            },
            Mark::Stroke {
                points,
                closed,
                width,
                color,
                dash,
            } => Mark::Stroke {
                points: points.iter().map(|p| unit(*p)).collect(),
                closed,
                width,
                color,
                dash,
            },
            Mark::Text(t) => Mark::Text(Text {
                at: unit(t.at),
                ..t
            }),
        })
        .collect()
}

fn existing_swatch() -> Vec<Mark> {
    swatch(|items, rings| {
        items.push(Item::fill(0, rings.to_vec(), WALL_FILL));
        outline(items, 0, rings, WALL_LINE, Rgb::BLACK, None);
    })
}

fn demolition_swatch() -> Vec<Mark> {
    swatch(|items, rings| demolition_marks(items, 0, rings, 1.0))
}

fn material_swatch(material: Option<WallMaterial>) -> Vec<Mark> {
    swatch(|items, rings| material_marks(items, 0, rings, Some(material), 1.0))
}

fn infill_swatch() -> Vec<Mark> {
    swatch(|items, rings| material_marks(items, 0, rings, None, 1.0))
}

// ---------------------------------------------------------------- рисование

/// Обводка контуров.
fn outline(
    items: &mut Vec<Item>,
    layer: usize,
    rings: &[Vec<DVec2>],
    width: f64,
    color: Rgb,
    dash: Option<(f64, f64)>,
) {
    for ring in rings {
        items.push(Item {
            layer,
            shape: Mark::Stroke {
                points: ring.clone(),
                closed: true,
                width,
                color,
                dash,
            },
        });
    }
}

/// Штриховка контуров; `spacing` — шаг на бумаге, `k` — знаменатель масштаба.
fn hatch_lines(
    items: &mut Vec<Item>,
    layer: usize,
    rings: &[Vec<DVec2>],
    angle_deg: f64,
    spacing: f64,
    k: f64,
    color: Rgb,
) {
    for (a, b) in hatch(rings, angle_deg.to_radians(), spacing * k) {
        items.push(Item::line(layer, a, b, HATCH_LINE, color));
    }
}

/// Демонтируемые конструкции: красная штриховка и штриховой контур.
fn demolition_marks(items: &mut Vec<Item>, layer: usize, rings: &[Vec<DVec2>], k: f64) {
    hatch_lines(items, layer, rings, 45.0, 1.5, k, DEMOLISH);
    outline(items, layer, rings, 0.35, DEMOLISH, Some(DEMOLISH_DASH));
}

/// Возводимые конструкции: заливка и штриховка материала. `material: None` — закладка проёма.
fn material_marks(
    items: &mut Vec<Item>,
    layer: usize,
    rings: &[Vec<DVec2>],
    material: Option<Option<WallMaterial>>,
    k: f64,
) {
    let (fill, hatches): (Rgb, &[(f64, f64)]) = match material {
        Some(Some(WallMaterial::AeratedConcrete)) => (Rgb::hex(0xDDE5EE), &[(45.0, 2.5)]),
        Some(Some(WallMaterial::Gypsum)) => (Rgb::hex(0xF3EBD2), &[(45.0, 2.5), (-45.0, 2.5)]),
        Some(Some(WallMaterial::Brick)) => (Rgb::hex(0xF1D9D0), &[(45.0, 1.2)]),
        Some(None) => (Rgb::hex(0xDCEFE5), &[(-45.0, 2.5)]),
        None => (Rgb::WHITE, &[(-45.0, 1.2)]),
    };
    items.push(Item::fill(layer, rings.to_vec(), fill));
    for (angle, spacing) in hatches {
        hatch_lines(items, layer, rings, *angle, *spacing, k, Rgb::BLACK);
    }
    outline(items, layer, rings, WALL_LINE, Rgb::BLACK, None);
}

/// Перо листа для символов проёмов.
struct SheetPen<'a> {
    items: &'a mut Vec<Item>,
    layer: usize,
    color: Rgb,
}

impl Pen for SheetPen<'_> {
    fn line(&mut self, a: DVec2, b: DVec2, weight: f64) {
        self.items.push(Item::line(
            self.layer,
            a,
            b,
            SYMBOL_LINE * weight,
            self.color,
        ));
    }

    fn dashed(&mut self, a: DVec2, b: DVec2, weight: f64) {
        self.items.push(Item {
            layer: self.layer,
            shape: Mark::Stroke {
                points: vec![a, b],
                closed: false,
                width: SYMBOL_LINE * weight,
                color: self.color,
                dash: Some((1.5, 1.0)),
            },
        });
    }

    fn faint(&mut self, points: &[DVec2], weight: f64) {
        self.items.push(Item::polyline(
            self.layer,
            points.to_vec(),
            false,
            SYMBOL_LINE * weight,
            self.color,
        ));
    }
}

/// План в видовом экране: примитивы в мм плана, толщины и текст — в мм бумаги.
struct Plot<'a> {
    sheet: &'a mut Sheet,
    items: Vec<Item>,
    /// Знаменатель масштаба.
    k: f64,
}

impl Plot<'_> {
    fn text(&mut self, layer: usize, at: DVec2, text: &str, height: f64, align: Align, color: Rgb) {
        // Точка `at` — середина строки по высоте прописных.
        self.items.push(Item::text(
            layer,
            Text {
                at: at + DVec2::new(0.0, height * self.k / 2.0),
                text: text.to_owned(),
                height,
                align,
                angle: 0.0,
                color,
            },
        ));
    }

    fn walls(&mut self, layer: &str, shapes: &[Shape]) {
        let layer = self.sheet.layer(layer);
        for shape in shapes {
            let rings = rings(shape);
            self.items.push(Item::fill(layer, rings.clone(), WALL_FILL));
            outline(&mut self.items, layer, &rings, WALL_LINE, Rgb::BLACK, None);
        }
    }

    fn demolished(&mut self, rings: &[Vec<DVec2>]) {
        if rings.is_empty() {
            return;
        }
        let layer = self.sheet.layer(LAYER_DEMOLITION);
        demolition_marks(&mut self.items, layer, rings, self.k);
    }

    fn material(&mut self, rings: &[Vec<DVec2>], material: Option<Option<WallMaterial>>) {
        if rings.is_empty() {
            return;
        }
        let layer = self.sheet.layer(LAYER_CONSTRUCTION);
        material_marks(&mut self.items, layer, rings, material, self.k);
    }

    fn openings(
        &mut self,
        doc: &Document,
        derived: &Derived,
        keep: impl Fn(&op_model::Opening) -> bool,
    ) {
        let layer = self.sheet.layer(LAYER_OPENINGS);
        for o in doc
            .openings
            .iter()
            .filter(|o| derived.visible.contains(&o.layer) && keep(o))
        {
            if let Some(g) = geometry::opening(doc, derived, o) {
                let mut pen = SheetPen {
                    items: &mut self.items,
                    layer,
                    color: Rgb::BLACK,
                };
                opening_lines(&mut pen, &g, o.kind, o.hinge_at_end, o.swing);
            }
        }
    }

    /// Подписи помещений: «номер. название» и (если нужно) площадь.
    fn room_labels(&mut self, doc: &Document, rooms: &[RoomRow], area: bool) {
        let layer = self.sheet.layer(LAYER_ROOMS);
        let numbers: HashMap<&str, usize> = rooms
            .iter()
            .enumerate()
            .map(|(i, r)| (r.id.as_str(), i + 1))
            .collect();
        for r in doc.rooms.iter() {
            let Some(row) = rooms.iter().find(|row| row.id == r.id) else {
                continue;
            };
            let p = DVec2::new(r.x, r.y);
            let k = self.k;
            let name = match numbers.get(r.id.as_str()) {
                Some(n) if area => format!("{n}. {}", r.name),
                _ => r.name.clone(),
            };
            // Подпись ужимается по ширине помещения.
            let fit = |text: &str, height: f64| {
                row.width
                    .map_or(height, |w| fit_height(text, height, w * 0.9 / k))
            };
            if area {
                let h = fit(&name, NAME_HEIGHT);
                self.text(
                    layer,
                    p - DVec2::new(0.0, h * 0.8 * k),
                    &name,
                    h,
                    Align::Center,
                    Rgb::BLACK,
                );
                let value = row
                    .area
                    .map_or_else(|| "контур не замкнут".to_owned(), geometry::format_area);
                self.text(
                    layer,
                    p + DVec2::new(0.0, SMALL_HEIGHT * 0.9 * k),
                    &value,
                    SMALL_HEIGHT,
                    Align::Center,
                    Rgb::BLACK,
                );
            } else {
                let h = fit(&name, SMALL_HEIGHT);
                self.text(layer, p, &name, h, Align::Center, Rgb::hex(0x555555));
            }
        }
    }

    fn dimensions(&mut self, doc: &Document, derived: &Derived) {
        let layer = self.sheet.layer(LAYER_DIMENSIONS);
        for d in doc
            .dimensions
            .iter()
            .filter(|d| derived.visible.contains(&d.layer))
        {
            if let Some(g) = geometry::dimension(derived, d) {
                self.dimension(layer, &g);
            }
        }
    }

    /// Размер по ГОСТ 2.307: выносные линии за размерную на 2 мм, засечки 45°,
    /// число над размерной линией.
    fn dimension(&mut self, layer: usize, g: &DimensionGeometry) {
        let k = self.k;
        let side = if (g.qa - g.a).dot(g.normal) < 0.0 {
            -1.0
        } else {
            1.0
        };
        let over = g.normal * side * 2.0 * k;
        for (p, q) in [(g.a, g.qa), (g.b, g.qb)] {
            self.items
                .push(Item::line(layer, p, q + over, THIN, Rgb::BLACK));
        }
        self.items.push(Item::line(
            layer,
            g.qa - g.dir * 1.5 * k,
            g.qb + g.dir * 1.5 * k,
            THIN,
            Rgb::BLACK,
        ));
        let tick = (g.dir + g.normal).normalize_or_zero() * 1.2 * k;
        for q in [g.qa, g.qb] {
            self.items
                .push(Item::line(layer, q - tick, q + tick, THICK, Rgb::BLACK));
        }
        let (angle, up) = geometry::readable(g.dir);
        self.items.push(Item::text(
            layer,
            Text {
                at: g.qa.midpoint(g.qb) + up * 0.8 * k,
                text: geometry::format_length(g.length),
                height: SMALL_HEIGHT,
                align: Align::Center,
                angle,
                color: Rgb::BLACK,
            },
        ));
    }

    fn leaders(&mut self, doc: &Document, derived: &Derived) {
        let layer = self.sheet.layer(LAYER_NOTES);
        let k = self.k;
        for l in doc
            .leaders
            .iter()
            .filter(|l| derived.visible.contains(&l.layer))
        {
            let target = DVec2::new(l.x, l.y);
            let shelf = DVec2::new(l.text_x, l.text_y);
            let width = SheetFont::shared().map_or(0.0, |f| f.text_width(&l.text, SMALL_HEIGHT));
            self.items
                .push(Item::line(layer, target, shelf, THIN, Rgb::BLACK));
            self.items.push(Item::line(
                layer,
                shelf,
                shelf + DVec2::X * (width + 2.0) * k,
                THIN,
                Rgb::BLACK,
            ));
            let dot: Vec<DVec2> = (0..12)
                .map(|i| {
                    let t = f64::from(i) / 12.0 * std::f64::consts::TAU;
                    target + DVec2::new(t.cos(), t.sin()) * 0.6 * k
                })
                .collect();
            self.items.push(Item::fill(layer, vec![dot], Rgb::BLACK));
            self.items.push(Item::text(
                layer,
                Text {
                    at: shelf + DVec2::new(1.0, -0.8) * k,
                    text: l.text.clone(),
                    height: SMALL_HEIGHT,
                    align: Align::Left,
                    angle: 0.0,
                    color: Rgb::BLACK,
                },
            ));
        }
    }

    /// Электроточки с подписями рядом (высота, марка).
    fn devices(
        &mut self,
        devices: &[&op_model::Device],
        label: impl Fn(&op_model::Device) -> Option<String>,
    ) {
        let layer = self.sheet.layer(LAYER_ELECTRIC);
        for d in devices {
            for m in electric::plan_marks(d) {
                self.items.push(Item {
                    layer,
                    shape: Mark::Stroke {
                        points: m.points,
                        closed: m.closed,
                        width: SYMBOL_LINE,
                        color: Rgb::BLACK,
                        dash: m.dashed.then_some((1.5, 1.0)),
                    },
                });
            }
            if let Some(text) = label(d) {
                let at = electric::to_plan(d, DVec2::new(0.0, electric::SYMBOL * 0.9));
                self.text(
                    layer,
                    at,
                    &text,
                    SMALL_HEIGHT * 0.8,
                    Align::Center,
                    Rgb::BLACK,
                );
            }
        }
    }

    /// Связи выключателей со светильниками: тонкие штриховые линии.
    fn switch_links(&mut self, switches: &[&op_model::Device], lights: &[&op_model::Device]) {
        let layer = self.sheet.layer(LAYER_ELECTRIC);
        for s in switches {
            for id in s.keys.iter().flatten() {
                if let Some(l) = lights.iter().find(|l| &l.id == id) {
                    self.items.push(Item {
                        layer,
                        shape: Mark::Stroke {
                            points: vec![DVec2::new(s.x, s.y), DVec2::new(l.x, l.y)],
                            closed: false,
                            width: THIN,
                            color: Rgb::BLACK,
                            dash: Some((2.0, 1.0)),
                        },
                    });
                }
            }
        }
    }

    /// Трасса группы и её обозначение у начала.
    fn route(&mut self, path: &[DVec2], name: &str) {
        if path.len() < 2 {
            return;
        }
        let layer = self.sheet.layer(LAYER_ELECTRIC);
        self.items.push(Item::polyline(
            layer,
            path.to_vec(),
            false,
            0.35,
            Rgb::hex(0x1D4ED8),
        ));
        let at = path[0].midpoint(path[1]) + DVec2::new(0.0, -SMALL_HEIGHT * self.k);
        self.text(
            layer,
            at,
            name,
            SMALL_HEIGHT * 0.8,
            Align::Center,
            Rgb::hex(0x1D4ED8),
        );
    }

    /// Мебель: обозначения и номера позиций спецификации в центрах предметов.
    fn furniture(&mut self, doc: &Document, derived: &Derived, rows: &[FurnitureRow]) {
        let layer = self.sheet.layer(LAYER_FURNITURE);
        for f in doc
            .furniture
            .iter()
            .filter(|f| derived.visible.contains(&f.layer))
        {
            let marks = crate::furniture::plan_marks(f, None);
            if let Some(outline) = marks.first() {
                self.items
                    .push(Item::fill(layer, vec![outline.points.clone()], Rgb::WHITE));
            }
            for m in marks {
                self.items.push(Item {
                    layer,
                    shape: Mark::Stroke {
                        points: m.points,
                        closed: m.closed,
                        width: SYMBOL_LINE,
                        color: Rgb::BLACK,
                        dash: m.dashed.then_some((1.5, 1.0)),
                    },
                });
            }
            if let Some(n) = rows.iter().position(|r| r.ids.contains(&f.id)) {
                let at = DVec2::new(f.x, f.y);
                let r = SMALL_HEIGHT * 0.9 * self.k;
                let circle: Vec<DVec2> = (0..24)
                    .map(|i| {
                        let t = f64::from(i) / 24.0 * std::f64::consts::TAU;
                        at + DVec2::new(t.cos(), t.sin()) * r
                    })
                    .collect();
                self.items
                    .push(Item::fill(layer, vec![circle.clone()], Rgb::WHITE));
                self.items
                    .push(Item::polyline(layer, circle, true, THIN, Rgb::BLACK));
                self.text(
                    layer,
                    at,
                    &(n + 1).to_string(),
                    SMALL_HEIGHT * 0.8,
                    Align::Center,
                    Rgb::BLACK,
                );
            }
        }
    }

    /// Марки элементов ведомости рядом с ними.
    fn marks(&mut self, layer: &str, rows: &[Row], color: Rgb) {
        let layer = self.sheet.layer(layer);
        for r in rows {
            let at = r.at + r.out * 2.5 * self.k;
            self.text(layer, at, &r.mark, SMALL_HEIGHT, Align::Center, color);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table(rows: usize) -> Table {
        Table {
            title: "Экспликация".into(),
            columns: vec![Column::new("№", TITLE_WIDTH, Align::Left)],
            rows: (0..rows).map(|i| vec![Cell::text(i.to_string())]).collect(),
            footer: Some(vec![Cell::text("Итого")]),
        }
    }

    #[test]
    fn scale_is_standard_and_not_larger_than_1_50() {
        assert_eq!(pick_scale(10.0), 50);
        assert_eq!(pick_scale(50.0), 50);
        assert_eq!(pick_scale(51.0), 75);
        assert_eq!(pick_scale(150.0), 200);
        assert_eq!(pick_scale(1e9), 1000);
    }

    #[test]
    fn long_table_is_split_with_footer_in_the_tail() {
        let t = table(30);
        let (head, tail) = split(&t, 100.0).unwrap();
        assert!(head.height() <= 100.0);
        assert!(head.footer.is_none());
        assert_eq!(head.rows.len() + tail.rows.len(), 30);
        assert_eq!(tail.footer, t.footer);
        assert!(tail.title.ends_with("(продолжение)"));
        // Меньше двух строк не отрезается.
        assert!(split(&t, 25.0).is_none());
    }

    #[test]
    fn tables_flow_onto_continuation_sheets() {
        let size = DVec2::new(297.0, 210.0);
        let placement = place(size, vec![table(60)], DVec2::new(3000.0, 4000.0));
        assert!(!placement.tables.is_empty(), "начало таблицы — на листе");
        let pages = continuation(size, placement.overflow);
        assert!(!pages.is_empty());
        let rows: usize = placement
            .tables
            .iter()
            .chain(pages.iter().flatten())
            .map(|(t, _)| t.rows.len())
            .sum();
        assert_eq!(rows, 60, "строки не теряются");
        let (lo, hi) = frame::inner(size);
        for (t, at) in pages.iter().flatten() {
            assert!(at.x >= lo.x && at.x + t.width() <= hi.x + 1e-9);
            assert!(at.y >= lo.y && at.y + t.height() <= hi.y - TITLE_HEIGHT + 1e-9);
        }
    }

    #[test]
    fn portrait_sheet_puts_tables_under_the_plan() {
        let size = DVec2::new(297.0, 420.0);
        let placement = place(size, vec![table(5)], DVec2::new(6000.0, 6000.0));
        let (_, at) = &placement.tables[0];
        assert!(at.y > placement.plan.1.y - 1e-9, "таблица под планом");
        assert!(placement.overflow.is_empty());
    }
}
