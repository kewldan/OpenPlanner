//! Смета из модели: объёмы по правилам позиций прайса (площади, перегородки,
//! точки, трубы, отделка, расход), норма и запас, ручные объёмы, группировка по
//! разделам и помещениям, список закупок; прайс в CSV и XLSX и обратно.
//!
//! Объёмы, которые можно отнести к помещению (площади, точки, мебель, отделка),
//! считаются по помещениям; остальные (демонтаж, перегородки, кабель, трубы) —
//! строкой без помещения.

use std::collections::HashMap;

use glam::DVec2;
use op_calc::estimate as calc;
use op_export::{
    Align,
    table::{Cell, Column, Table},
    xlsx::Worksheet,
};
use op_model::{
    DeviceGroup, Document, MaterialKind, Measure, PipeSystem, PriceItem, PriceKind, PricePoint,
    Surface, WallMaterial,
};

use crate::{
    derived::Derived,
    protocol::{EstimateGroup, EstimateLine, EstimateReport, Purchase, ViewSummary},
};

/// Строки без помещения в группировке по помещениям.
pub const NO_ROOM: &str = "Общие работы";

// ---------------------------------------------------------------- типовой прайс

type Template = (
    &'static str,
    &'static str,
    &'static str,
    PriceKind,
    &'static str,
    f64,
    fn() -> Measure,
    f64,
    f64,
);

const W: PriceKind = PriceKind::Work;
const M: PriceKind = PriceKind::Material;

/// Типовой прайс: код, раздел, наименование, вид, единица, цена (примерная),
/// объём, норма, запас.
// Таблица читается построчно: rustfmt разворачивает каждую позицию на 15 строк.
#[rustfmt::skip]
const TEMPLATE: &[Template] = &[
    ("Д-01", "Демонтаж", "Демонтаж перегородок", W, "м²", 650.0, || Measure::Demolition, 1.0, 0.0),
    ("Д-02", "Демонтаж", "Вынос и вывоз строительного мусора", W, "м²", 250.0, || Measure::Demolition, 1.0, 0.0),
    ("П-01", "Перегородки", "Кладка перегородок из газобетона", W, "м²", 1300.0, || Measure::Partitions { material: Some(WallMaterial::AeratedConcrete) }, 1.0, 0.0),
    ("П-02", "Перегородки", "Перегородки из ГКЛ на металлическом каркасе", W, "м²", 1200.0, || Measure::Partitions { material: Some(WallMaterial::Gypsum) }, 1.0, 0.0),
    ("П-03", "Перегородки", "Кладка перегородок из кирпича", W, "м²", 1700.0, || Measure::Partitions { material: Some(WallMaterial::Brick) }, 1.0, 0.0),
    ("П-04", "Перегородки", "Блок газобетонный 100 мм", M, "м²", 700.0, || Measure::Partitions { material: Some(WallMaterial::AeratedConcrete) }, 1.0, 0.05),
    ("П-05", "Перегородки", "Лист ГКЛ 12,5 мм (две стороны)", M, "м²", 190.0, || Measure::Partitions { material: Some(WallMaterial::Gypsum) }, 2.0, 0.1),
    ("П-06", "Перегородки", "Устройство проёма", W, "шт.", 4500.0, || Measure::NewOpenings, 1.0, 0.0),
    ("Э-01", "Электрика", "Установка розетки или вывода", W, "шт.", 450.0, || Measure::Devices { group: Some(DeviceGroup::Outlet) }, 1.0, 0.0),
    ("Э-02", "Электрика", "Установка выключателя", W, "шт.", 400.0, || Measure::Devices { group: Some(DeviceGroup::Switch) }, 1.0, 0.0),
    ("Э-03", "Электрика", "Установка светильника", W, "шт.", 900.0, || Measure::Devices { group: Some(DeviceGroup::Light) }, 1.0, 0.0),
    ("Э-04", "Электрика", "Сборка электрощита", W, "шт.", 9000.0, || Measure::Devices { group: Some(DeviceGroup::Panel) }, 1.0, 0.0),
    ("Э-05", "Электрика", "Прокладка кабеля", W, "м", 120.0, || Measure::Cable, 1.0, 0.0),
    ("Э-06", "Электрика", "Кабель ВВГнг-LS 3×2,5", M, "м", 115.0, || Measure::Cable, 1.0, 0.1),
    ("С-01", "Сантехника", "Монтаж вывода водоснабжения или канализации", W, "шт.", 1800.0, || Measure::Outlets { system: None }, 1.0, 0.0),
    ("С-02", "Сантехника", "Прокладка трубопровода", W, "м", 450.0, || Measure::Pipes { system: None }, 1.0, 0.0),
    ("С-03", "Сантехника", "Установка сантехнического прибора", W, "шт.", 3500.0, || Measure::Fixtures, 1.0, 0.0),
    ("О-01", "Отделка", "Укладка плитки на пол", W, "м²", 1900.0, || Measure::Finish { surface: Some(Surface::Floor), kind: Some(MaterialKind::Tile) }, 1.0, 0.0),
    ("О-02", "Отделка", "Облицовка стен плиткой", W, "м²", 2100.0, || Measure::Finish { surface: Some(Surface::Walls), kind: Some(MaterialKind::Tile) }, 1.0, 0.0),
    ("О-03", "Отделка", "Укладка ламината или доски", W, "м²", 550.0, || Measure::Finish { surface: Some(Surface::Floor), kind: Some(MaterialKind::Laminate) }, 1.0, 0.0),
    ("О-04", "Отделка", "Окраска стен", W, "м²", 450.0, || Measure::Finish { surface: Some(Surface::Walls), kind: Some(MaterialKind::Paint) }, 1.0, 0.0),
    ("О-05", "Отделка", "Поклейка обоев", W, "м²", 420.0, || Measure::Finish { surface: Some(Surface::Walls), kind: Some(MaterialKind::Wallpaper) }, 1.0, 0.0),
    ("О-06", "Отделка", "Окраска потолка", W, "м²", 550.0, || Measure::Finish { surface: Some(Surface::Ceiling), kind: Some(MaterialKind::Paint) }, 1.0, 0.0),
    ("О-07", "Отделка", "Натяжной потолок", W, "м²", 950.0, || Measure::Finish { surface: Some(Surface::Ceiling), kind: Some(MaterialKind::Sheet) }, 1.0, 0.0),
    ("О-08", "Отделка", "Грунтовка стен", M, "л", 110.0, || Measure::WallArea, 0.15, 0.1),
    ("О-09", "Отделка", "Плитка и керамогранит", M, "м²", 1500.0, || Measure::Finish { surface: None, kind: Some(MaterialKind::Tile) }, 1.0, 0.1),
    ("О-10", "Отделка", "Плиточный клей", M, "кг", 22.0, || Measure::Consumption { name: "Клей".into(), material: None }, 1.0, 0.0),
    ("О-11", "Отделка", "Затирка", M, "кг", 350.0, || Measure::Consumption { name: "Затирка".into(), material: None }, 1.0, 0.0),
    ("О-12", "Отделка", "Краска интерьерная", M, "л", 450.0, || Measure::Consumption { name: "Краска".into(), material: None }, 1.0, 0.0),
    ("О-13", "Отделка", "Обои", M, "рул.", 1800.0, || Measure::Consumption { name: "Обои".into(), material: None }, 1.0, 0.0),
    ("О-14", "Отделка", "Ламинат или доска", M, "м²", 1100.0, || Measure::Finish { surface: Some(Surface::Floor), kind: Some(MaterialKind::Laminate) }, 1.0, 0.07),
    ("М-01", "Мебель", "Сборка и установка мебели", W, "шт.", 1500.0, || Measure::Furniture, 1.0, 0.0),
    ("Р-01", "Общие работы", "Уборка после ремонта", W, "м²", 120.0, || Measure::FloorArea, 1.0, 0.0),
];

/// Типовой прайс с примерными ценами; идентификаторы — от кода.
#[must_use]
pub fn template() -> Vec<PriceItem> {
    TEMPLATE
        .iter()
        .map(
            |(code, section, name, kind, unit, price, measure, rate, reserve)| PriceItem {
                id: format!("price-{code}"),
                code: (*code).into(),
                section: (*section).into(),
                name: (*name).into(),
                kind: *kind,
                unit: (*unit).into(),
                price: *price,
                measure: measure(),
                rate: *rate,
                reserve: *reserve,
                history: Vec::new(),
            },
        )
        .collect()
}

// ---------------------------------------------------------------- правила объёма

fn group_token(g: DeviceGroup) -> &'static str {
    match g {
        DeviceGroup::Outlet => "outlet",
        DeviceGroup::Switch => "switch",
        DeviceGroup::Light => "light",
        DeviceGroup::Panel => "panel",
    }
}

fn system_token(s: PipeSystem) -> &'static str {
    match s {
        PipeSystem::Cold => "cold",
        PipeSystem::Hot => "hot",
        PipeSystem::Drain => "drain",
    }
}

fn material_token(m: WallMaterial) -> &'static str {
    match m {
        WallMaterial::AeratedConcrete => "aeratedConcrete",
        WallMaterial::Gypsum => "gypsum",
        WallMaterial::Brick => "brick",
    }
}

fn surface_token(s: Surface) -> &'static str {
    match s {
        Surface::Floor => "floor",
        Surface::Ceiling => "ceiling",
        Surface::Walls => "walls",
    }
}

fn kind_token(k: MaterialKind) -> &'static str {
    match k {
        MaterialKind::Tile => "tile",
        MaterialKind::Laminate => "laminate",
        MaterialKind::Paint => "paint",
        MaterialKind::Wallpaper => "wallpaper",
        MaterialKind::Sheet => "sheet",
    }
}

fn find<T: Copy>(all: &[T], token: &str, name: fn(T) -> &'static str) -> Option<T> {
    all.iter().copied().find(|v| name(*v) == token)
}

const GROUPS: [DeviceGroup; 4] = [
    DeviceGroup::Outlet,
    DeviceGroup::Switch,
    DeviceGroup::Light,
    DeviceGroup::Panel,
];
const SURFACES: [Surface; 3] = [Surface::Floor, Surface::Ceiling, Surface::Walls];
const KINDS: [MaterialKind; 5] = [
    MaterialKind::Tile,
    MaterialKind::Laminate,
    MaterialKind::Paint,
    MaterialKind::Wallpaper,
    MaterialKind::Sheet,
];

/// Правило объёма в одну строку — для CSV и XLSX: `finish:floor:tile`,
/// `devices:outlet`, `consumption:Клей`, `manual`.
#[must_use]
pub fn measure_token(m: &Measure) -> String {
    let opt =
        |base: &str, v: Option<&str>| v.map_or_else(|| base.to_owned(), |v| format!("{base}:{v}"));
    match m {
        Measure::Manual => "manual".into(),
        Measure::FloorArea => "floorArea".into(),
        Measure::CeilingArea => "ceilingArea".into(),
        Measure::WallArea => "wallArea".into(),
        Measure::Perimeter => "perimeter".into(),
        Measure::Demolition => "demolition".into(),
        Measure::Partitions { material } => opt("partitions", material.map(material_token)),
        Measure::NewOpenings => "newOpenings".into(),
        Measure::Devices { group } => opt("devices", group.map(group_token)),
        Measure::Cable => "cable".into(),
        Measure::Outlets { system } => opt("outlets", system.map(system_token)),
        Measure::Pipes { system } => opt("pipes", system.map(system_token)),
        Measure::Fixtures => "fixtures".into(),
        Measure::Furniture => "furniture".into(),
        Measure::Finish { surface, kind } => format!(
            "finish:{}:{}",
            surface.map_or("*", surface_token),
            kind.map_or("*", kind_token)
        ),
        Measure::Consumption { name, material } => match material {
            Some(m) => format!("consumption:{name}:{m}"),
            None => format!("consumption:{name}"),
        },
    }
}

/// Правило объёма из строки [`measure_token`]; неизвестное — `None`.
#[must_use]
pub fn parse_measure(token: &str) -> Option<Measure> {
    let token = token.trim();
    let mut parts = token.splitn(3, ':');
    let head = parts.next()?;
    let a = parts.next().filter(|s| !s.is_empty());
    let b = parts.next().filter(|s| !s.is_empty());
    // «*» в правиле отделки — любое значение.
    let (fa, fb) = (a.filter(|s| *s != "*"), b.filter(|s| *s != "*"));
    Some(match head {
        "manual" | "" => Measure::Manual,
        "floorArea" => Measure::FloorArea,
        "ceilingArea" => Measure::CeilingArea,
        "wallArea" => Measure::WallArea,
        "perimeter" => Measure::Perimeter,
        "demolition" => Measure::Demolition,
        "partitions" => Measure::Partitions {
            material: match a {
                Some(t) => Some(find(&WallMaterial::ALL, t, material_token)?),
                None => None,
            },
        },
        "newOpenings" => Measure::NewOpenings,
        "devices" => Measure::Devices {
            group: match a {
                Some(t) => Some(find(&GROUPS, t, group_token)?),
                None => None,
            },
        },
        "cable" => Measure::Cable,
        "outlets" | "pipes" => {
            let system = match a {
                Some(t) => Some(find(&PipeSystem::ALL, t, system_token)?),
                None => None,
            };
            if head == "outlets" {
                Measure::Outlets { system }
            } else {
                Measure::Pipes { system }
            }
        }
        "fixtures" => Measure::Fixtures,
        "furniture" => Measure::Furniture,
        "finish" => Measure::Finish {
            surface: match fa {
                Some(t) => Some(find(&SURFACES, t, surface_token)?),
                None => None,
            },
            kind: match fb {
                Some(t) => Some(find(&KINDS, t, kind_token)?),
                None => None,
            },
        },
        "consumption" => Measure::Consumption {
            name: a?.to_owned(),
            material: b.map(str::to_owned),
        },
        _ => return None,
    })
}

// ---------------------------------------------------------------- объёмы вида

/// Помещение вида: идентификатор, название, область.
struct RoomRef {
    id: String,
    name: String,
    region: usize,
}

/// Всё, из чего берутся объёмы: помещения, отделка, электрика, изменения.
struct Context<'a> {
    doc: &'a Document,
    derived: &'a Derived,
    summary: &'a ViewSummary,
    rooms: Vec<RoomRef>,
    surfaces: HashMap<String, crate::protocol::RoomSurfaces>,
    finishes: HashMap<String, crate::protocol::FinishInfo>,
    cable: f64,
}

impl<'a> Context<'a> {
    fn new(doc: &'a Document, derived: &'a Derived, summary: &'a ViewSummary) -> Self {
        let mut used = std::collections::HashSet::new();
        let rooms = doc
            .rooms
            .iter()
            .filter(|r| derived.visible.contains(&r.layer))
            .filter_map(|r| {
                let i = *derived.room_regions.get(&r.id)?;
                used.insert(i).then(|| RoomRef {
                    id: r.id.clone(),
                    name: r.name.clone(),
                    region: i,
                })
            })
            .collect();
        let report = crate::finish::report(doc, derived);
        let electrical = crate::electric::report(doc, derived);
        Self {
            doc,
            derived,
            summary,
            rooms,
            surfaces: report
                .rooms
                .into_iter()
                .map(|r| (r.id.clone(), r))
                .collect(),
            finishes: report
                .finishes
                .into_iter()
                .map(|f| (f.id.clone(), f))
                .collect(),
            cable: electrical.circuits.iter().map(|c| c.length).sum(),
        }
    }

    /// Помещение, в котором лежит точка.
    fn room_at(&self, p: DVec2) -> Option<&str> {
        let i = op_geom::shapes::region_at(&self.derived.regions, p)?;
        self.rooms
            .iter()
            .find(|r| r.region == i)
            .map(|r| r.id.as_str())
    }

    fn shown(&self, layer: &str) -> bool {
        self.derived.visible.contains(layer)
    }

    /// Объём правила: по помещениям (`Some`) или строкой без помещения (`None`).
    fn measure(&self, m: &Measure) -> Vec<(Option<String>, f64)> {
        let mut by_room: HashMap<String, f64> = HashMap::new();
        let mut add = |room: Option<&str>, v: f64| {
            if let Some(room) = room {
                *by_room.entry(room.to_owned()).or_default() += v;
            }
        };
        let doc = self.doc;
        let free = match m {
            Measure::Manual => return vec![(None, 0.0)],
            Measure::Demolition => Some(self.summary.demolished_area),
            Measure::Partitions { material } => Some(
                self.summary
                    .new_walls
                    .iter()
                    .filter(|w| material.is_none() || w.material == *material)
                    .map(|w| w.area)
                    .sum(),
            ),
            Measure::NewOpenings => Some(f64::from(self.summary.openings_added)),
            Measure::Cable => Some(self.cable),
            Measure::Pipes { system } => Some(
                doc.pipes
                    .iter()
                    .filter(|p| self.shown(&p.layer))
                    .filter(|p| system.is_none_or(|s| p.system == s))
                    .map(|p| crate::plumbing::length(p) / 1000.0)
                    .sum(),
            ),
            _ => None,
        };
        if let Some(v) = free {
            return vec![(None, v)];
        }
        match m {
            Measure::FloorArea | Measure::CeilingArea | Measure::WallArea | Measure::Perimeter => {
                for (id, s) in &self.surfaces {
                    let v = match m {
                        Measure::FloorArea => s.floor,
                        Measure::CeilingArea => s.ceiling,
                        Measure::WallArea => s.walls_net,
                        _ => s.perimeter,
                    };
                    add(Some(id), v);
                }
            }
            Measure::Devices { group } => {
                for d in doc.devices.iter().filter(|d| self.shown(&d.layer)) {
                    let g = if d.kind.is_light() {
                        DeviceGroup::Light
                    } else if crate::electric::is_switch(d.kind) {
                        DeviceGroup::Switch
                    } else if d.kind == op_model::DeviceKind::Panel {
                        DeviceGroup::Panel
                    } else if crate::electric::is_outlet(d.kind) {
                        DeviceGroup::Outlet
                    } else {
                        continue;
                    };
                    if group.is_none_or(|x| x == g) {
                        add(self.room_at(DVec2::new(d.x, d.y)), 1.0);
                    }
                }
            }
            Measure::Outlets { system } => {
                for p in doc.plumbing.iter().filter(|p| {
                    self.shown(&p.layer)
                        && p.kind == op_model::PlumbingKind::Outlet
                        && system.is_none_or(|s| p.system == s)
                }) {
                    add(self.room_at(DVec2::new(p.x, p.y)), 1.0);
                }
            }
            Measure::Fixtures | Measure::Furniture => {
                for f in doc.furniture.iter().filter(|f| self.shown(&f.layer)) {
                    if matches!(m, Measure::Furniture) || crate::plumbing::is_fixture(f) {
                        add(self.room_at(DVec2::new(f.x, f.y)), 1.0);
                    }
                }
            }
            Measure::Finish { surface, kind } => {
                for f in &doc.finishes {
                    let material = crate::finish::material(doc, &f.material);
                    let fits = surface.is_none_or(|s| f.surface == s)
                        && kind.is_none_or(|k| material.as_ref().is_some_and(|m| m.kind == k));
                    if let (true, Some(info)) = (fits, self.finishes.get(&f.id)) {
                        add(Some(&f.room), info.area);
                    }
                }
            }
            Measure::Consumption { name, material } => {
                for f in doc
                    .finishes
                    .iter()
                    .filter(|f| material.as_ref().is_none_or(|m| &f.material == m))
                {
                    if let Some(info) = self.finishes.get(&f.id) {
                        let v: f64 = info
                            .quantities
                            .iter()
                            .filter(|q| &q.name == name)
                            .map(|q| q.amount)
                            .sum();
                        add(Some(&f.room), v);
                    }
                }
            }
            _ => {}
        }
        // Помещения — в порядке документа.
        self.rooms
            .iter()
            .filter_map(|r| by_room.get(&r.id).map(|v| (Some(r.id.clone()), *v)))
            .collect()
    }

    fn room_name(&self, id: Option<&str>) -> String {
        id.and_then(|id| self.rooms.iter().find(|r| r.id == id))
            .map_or_else(|| NO_ROOM.to_owned(), |r| r.name.clone())
    }
}

// ---------------------------------------------------------------- смета

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

fn add_to(groups: &mut Vec<EstimateGroup>, name: &str, line: &EstimateLine) {
    let at = match groups.iter().position(|g| g.name == name) {
        Some(i) => i,
        None => {
            groups.push(EstimateGroup {
                name: name.to_owned(),
                works: 0.0,
                materials: 0.0,
                total: 0.0,
            });
            groups.len() - 1
        }
    };
    let g = &mut groups[at];
    match line.kind {
        PriceKind::Work => g.works = round2(g.works + line.cost),
        PriceKind::Material => g.materials = round2(g.materials + line.cost),
    }
    g.total = round2(g.works + g.materials);
}

/// Смета вида. `summary` — сводка того же вида (демонтаж, новые перегородки, проёмы).
#[must_use]
pub fn report(doc: &Document, derived: &Derived, summary: &ViewSummary) -> EstimateReport {
    let ctx = Context::new(doc, derived, summary);
    let mut lines = Vec::new();
    for item in &doc.prices {
        let mut rows = ctx.measure(&item.measure);
        // Ручные объёмы строк, которых модель не дала (например, помещения без объёма).
        for o in doc.overrides.iter().filter(|o| o.item == item.id) {
            let known =
                o.room.is_none() || ctx.rooms.iter().any(|r| Some(&r.id) == o.room.as_ref());
            if known && !rows.iter().any(|(room, _)| *room == o.room) {
                rows.push((o.room.clone(), 0.0));
            }
        }
        let whole = calc::is_whole(&item.unit);
        for (room, measured) in rows {
            let manual = doc
                .overrides
                .iter()
                .find(|o| o.item == item.id && o.room == room);
            let quantity = manual.map_or_else(
                || calc::quantity(measured, item.rate, item.reserve, whole),
                |o| o.quantity,
            );
            if quantity <= 0.0 && manual.is_none() && item.measure != Measure::Manual {
                continue;
            }
            lines.push(EstimateLine {
                item: item.id.clone(),
                room_name: ctx.room_name(room.as_deref()),
                room,
                section: item.section.clone(),
                code: item.code.clone(),
                name: item.name.clone(),
                kind: item.kind,
                unit: item.unit.clone(),
                measured: round2(measured),
                quantity,
                price: item.price,
                cost: calc::cost(quantity, item.price),
                overridden: manual.is_some(),
            });
        }
    }
    let order: Vec<String> = ctx.rooms.iter().map(|r| r.name.clone()).collect();
    summarize(lines, &order)
}

/// Итоги по строкам: разделы, помещения (в порядке `order`, общие — в конце),
/// закупки, работы, материалы.
fn summarize(lines: Vec<EstimateLine>, order: &[String]) -> EstimateReport {
    let mut sections = Vec::new();
    let mut rooms = Vec::new();
    for line in &lines {
        add_to(&mut sections, &line.section, line);
        add_to(&mut rooms, &line.room_name, line);
    }
    rooms.sort_by_key(|g| {
        order
            .iter()
            .position(|n| *n == g.name)
            .unwrap_or(usize::MAX)
    });
    let mut purchases: Vec<Purchase> = Vec::new();
    for line in lines.iter().filter(|l| l.kind == PriceKind::Material) {
        match purchases.iter_mut().find(|p| p.item == line.item) {
            Some(p) => {
                p.quantity = round2(p.quantity + line.quantity);
                p.cost = calc::cost(p.quantity, p.price);
            }
            None => purchases.push(Purchase {
                item: line.item.clone(),
                code: line.code.clone(),
                name: line.name.clone(),
                unit: line.unit.clone(),
                quantity: line.quantity,
                price: line.price,
                cost: line.cost,
            }),
        }
    }
    let works = round2(sections.iter().map(|g| g.works).sum());
    let materials = round2(sections.iter().map(|g| g.materials).sum());
    EstimateReport {
        lines,
        sections,
        rooms,
        purchases,
        works,
        materials,
        total: round2(works + materials),
    }
}

/// Смета из смет этажей: помещения получают название этажа впереди.
#[must_use]
pub fn combine(parts: Vec<(String, EstimateReport)>) -> EstimateReport {
    if parts.len() == 1 {
        return parts
            .into_iter()
            .next()
            .map(|(_, r)| r)
            .unwrap_or_else(|| summarize(Vec::new(), &[]));
    }
    let mut lines = Vec::new();
    let mut order = Vec::new();
    for (level, report) in parts {
        for g in &report.rooms {
            order.push(format!("{level} · {}", g.name));
        }
        lines.extend(report.lines.into_iter().map(|mut l| {
            l.room_name = format!("{level} · {}", l.room_name);
            l
        }));
    }
    summarize(lines, &order)
}

// ---------------------------------------------------------------- таблицы и файлы

const PRICE_HEADERS: [&str; 9] = [
    "Код",
    "Раздел",
    "Наименование",
    "Вид",
    "Ед.",
    "Цена, ₽",
    "Объём",
    "Норма",
    "Запас, %",
];

fn kind_name(k: PriceKind) -> &'static str {
    match k {
        PriceKind::Work => "Работа",
        PriceKind::Material => "Материал",
    }
}

/// Прайс таблицей (для XLSX и CSV).
#[must_use]
pub fn prices_table(prices: &[PriceItem]) -> Table {
    let widths = [16.0, 30.0, 70.0, 20.0, 12.0, 22.0, 40.0, 14.0, 14.0];
    Table {
        title: "Прайс".into(),
        columns: PRICE_HEADERS
            .iter()
            .zip(widths)
            .map(|(h, w)| Column::new(*h, w, Align::Left))
            .collect(),
        rows: prices
            .iter()
            .map(|p| {
                vec![
                    Cell::text(p.code.clone()),
                    Cell::text(p.section.clone()),
                    Cell::text(p.name.clone()),
                    Cell::text(kind_name(p.kind)),
                    Cell::text(p.unit.clone()),
                    Cell::number(p.price, 2),
                    Cell::text(measure_token(&p.measure)),
                    Cell::number(p.rate, 3),
                    Cell::number(p.reserve * 100.0, 1),
                ]
            })
            .collect(),
        footer: None,
    }
}

fn line_table(title: &str, lines: &[&EstimateLine], with_room: bool) -> Table {
    let total: f64 = lines.iter().map(|l| l.cost).sum();
    let place = if with_room {
        "Помещение"
    } else {
        "Раздел"
    };
    Table {
        title: title.to_owned(),
        columns: vec![
            Column::new("№", 10.0, Align::Center),
            Column::new("Код", 16.0, Align::Left),
            Column::new("Наименование", 70.0, Align::Left),
            Column::new(place, 35.0, Align::Left),
            Column::new("Ед.", 12.0, Align::Center),
            Column::new("Кол.", 20.0, Align::Right),
            Column::new("Цена, ₽", 22.0, Align::Right),
            Column::new("Сумма, ₽", 26.0, Align::Right),
        ],
        rows: lines
            .iter()
            .enumerate()
            .map(|(i, l)| {
                vec![
                    Cell::text((i + 1).to_string()),
                    Cell::text(l.code.clone()),
                    Cell::text(l.name.clone()),
                    Cell::text(if with_room {
                        l.room_name.clone()
                    } else {
                        l.section.clone()
                    }),
                    Cell::text(l.unit.clone()),
                    Cell::number(l.quantity, 2),
                    Cell::number(l.price, 2),
                    Cell::number(l.cost, 2),
                ]
            })
            .collect(),
        footer: Some(vec![
            Cell::text(""),
            Cell::text(""),
            Cell::text("Итого"),
            Cell::text(""),
            Cell::text(""),
            Cell::text(""),
            Cell::text(""),
            Cell::number(total, 2),
        ]),
    }
}

fn totals_table(report: &EstimateReport) -> Table {
    Table {
        title: "Итог сметы".into(),
        columns: vec![
            Column::new("", 60.0, Align::Left),
            Column::new("Сумма, ₽", 30.0, Align::Right),
        ],
        rows: vec![
            vec![Cell::text("Работы"), Cell::number(report.works, 2)],
            vec![Cell::text("Материалы"), Cell::number(report.materials, 2)],
        ],
        footer: Some(vec![Cell::text("Всего"), Cell::number(report.total, 2)]),
    }
}

fn purchases_table(report: &EstimateReport) -> Table {
    Table {
        title: "Список закупок".into(),
        columns: vec![
            Column::new("Код", 16.0, Align::Left),
            Column::new("Наименование", 80.0, Align::Left),
            Column::new("Ед.", 12.0, Align::Center),
            Column::new("Кол.", 20.0, Align::Right),
            Column::new("Цена, ₽", 22.0, Align::Right),
            Column::new("Сумма, ₽", 26.0, Align::Right),
        ],
        rows: report
            .purchases
            .iter()
            .map(|p| {
                vec![
                    Cell::text(p.code.clone()),
                    Cell::text(p.name.clone()),
                    Cell::text(p.unit.clone()),
                    Cell::number(p.quantity, 2),
                    Cell::number(p.price, 2),
                    Cell::number(p.cost, 2),
                ]
            })
            .collect(),
        footer: Some(vec![
            Cell::text(""),
            Cell::text("Итого"),
            Cell::text(""),
            Cell::text(""),
            Cell::text(""),
            Cell::number(report.purchases.iter().map(|p| p.cost).sum(), 2),
        ]),
    }
}

/// Таблицы сметы по разделам и итог.
fn section_tables(report: &EstimateReport) -> Vec<Table> {
    let mut out: Vec<Table> = report
        .sections
        .iter()
        .map(|s| {
            let lines: Vec<&EstimateLine> = report
                .lines
                .iter()
                .filter(|l| l.section == s.name)
                .collect();
            line_table(&s.name, &lines, true)
        })
        .collect();
    out.push(totals_table(report));
    out
}

/// Книга сметы: по разделам, по помещениям, закупки.
#[must_use]
pub fn worksheets(report: &EstimateReport) -> Vec<Worksheet> {
    let by_room = report
        .rooms
        .iter()
        .map(|g| {
            let lines: Vec<&EstimateLine> = report
                .lines
                .iter()
                .filter(|l| l.room_name == g.name)
                .collect();
            line_table(&g.name, &lines, false)
        })
        .collect();
    vec![
        Worksheet {
            name: "Смета".into(),
            tables: section_tables(report),
        },
        Worksheet {
            name: "По помещениям".into(),
            tables: by_room,
        },
        Worksheet {
            name: "Закупки".into(),
            tables: vec![purchases_table(report)],
        },
    ]
}

/// Смета в CSV: разделы, итог и список закупок.
#[must_use]
pub fn csv(report: &EstimateReport) -> String {
    let mut tables = section_tables(report);
    tables.push(purchases_table(report));
    op_export::csv::write(&op_export::csv::from_tables(&tables))
}

/// Прайс в CSV.
#[must_use]
pub fn prices_csv(prices: &[PriceItem]) -> String {
    let t = prices_table(prices);
    let mut rows = vec![t.columns.iter().map(|c| c.header.clone()).collect()];
    rows.extend(op_export::csv::from_tables(&[t]).into_iter().skip(2));
    op_export::csv::write(&rows)
}

fn number(text: &str) -> Option<f64> {
    let cleaned: String = text
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '\u{a0}' && *c != '₽')
        .map(|c| if c == ',' { '.' } else { c })
        .collect();
    if cleaned.is_empty() {
        None
    } else {
        cleaned.parse().ok()
    }
}

/// Итог импорта прайса.
#[derive(Debug, Clone, PartialEq)]
pub struct Imported {
    /// Новый прайс.
    pub prices: Vec<PriceItem>,
    /// Добавлено позиций.
    pub added: usize,
    /// Обновлено позиций.
    pub updated: usize,
}

/// Прайс из строк таблицы (CSV или первый лист XLSX). Позиции сопоставляются по
/// коду, без кода — по наименованию; изменившаяся цена уходит в историю с датой `date`.
///
/// # Errors
///
/// Нет строки заголовков с «Наименование» и «Цена» или ни одной позиции.
pub fn import(
    existing: &[PriceItem],
    rows: &[Vec<String>],
    date: &str,
    mut new_id: impl FnMut() -> String,
) -> Result<Imported, String> {
    let header_at = rows
        .iter()
        .position(|r| {
            let has = |p: &str| r.iter().any(|c| c.trim().to_lowercase().starts_with(p));
            has("наименование") && has("цена")
        })
        .ok_or("в таблице нет строки заголовков с «Наименование» и «Цена»")?;
    let header: Vec<String> = rows[header_at]
        .iter()
        .map(|c| c.trim().to_lowercase())
        .collect();
    let col = |prefix: &str| header.iter().position(|h| h.starts_with(prefix));
    let (code, section, name, kind, unit, price, measure, rate, reserve) = (
        col("код"),
        col("раздел"),
        col("наименование"),
        col("вид"),
        col("ед"),
        col("цена"),
        col("объём").or_else(|| col("объем")),
        col("норма"),
        col("запас"),
    );
    let mut prices = existing.to_vec();
    let (mut added, mut updated) = (0, 0);
    for row in &rows[header_at + 1..] {
        let cell = |i: Option<usize>| {
            i.and_then(|i| row.get(i))
                .map(|s| s.trim().to_owned())
                .unwrap_or_default()
        };
        let item_name = cell(name);
        let Some(item_price) = number(&cell(price)) else {
            continue;
        };
        if item_name.is_empty() || item_price < 0.0 {
            continue;
        }
        let item_code = cell(code);
        let parsed_measure = parse_measure(&cell(measure)).unwrap_or(Measure::Manual);
        let item_kind = if cell(kind).to_lowercase().starts_with("мат") {
            PriceKind::Material
        } else {
            PriceKind::Work
        };
        let found = prices.iter().position(|p| {
            if item_code.is_empty() {
                p.name.to_lowercase() == item_name.to_lowercase()
            } else {
                p.code == item_code
            }
        });
        let section_name = Some(cell(section)).filter(|s| !s.is_empty());
        let unit_name = Some(cell(unit)).filter(|s| !s.is_empty());
        let item_rate = number(&cell(rate)).filter(|v| *v >= 0.0);
        let item_reserve = number(&cell(reserve)).map(|v| (v / 100.0).max(0.0));
        match found {
            Some(i) => {
                let p = &mut prices[i];
                if (p.price - item_price).abs() > 1e-9 {
                    p.history.push(PricePoint {
                        date: date.to_owned(),
                        price: p.price,
                    });
                    p.price = item_price;
                }
                p.name = item_name;
                p.kind = item_kind;
                if let Some(s) = section_name {
                    p.section = s;
                }
                if let Some(u) = unit_name {
                    p.unit = u;
                }
                if measure.is_some() {
                    p.measure = parsed_measure;
                }
                if let Some(r) = item_rate {
                    p.rate = r;
                }
                if let Some(r) = item_reserve {
                    p.reserve = r;
                }
                updated += 1;
            }
            None => {
                prices.push(PriceItem {
                    id: new_id(),
                    code: item_code,
                    section: section_name.unwrap_or_else(|| "Прочее".into()),
                    name: item_name,
                    kind: item_kind,
                    unit: unit_name.unwrap_or_else(|| "шт.".into()),
                    price: item_price,
                    measure: parsed_measure,
                    rate: item_rate.unwrap_or(1.0),
                    reserve: item_reserve.unwrap_or(0.0),
                    history: Vec::new(),
                });
                added += 1;
            }
        }
    }
    if added + updated == 0 {
        return Err("в таблице нет позиций с наименованием и ценой".into());
    }
    Ok(Imported {
        prices,
        added,
        updated,
    })
}

/// Строки таблицы из файла прайса: XLSX (по сигнатуре ZIP) или CSV в UTF-8.
///
/// # Errors
///
/// Файл не читается.
pub fn read_table(bytes: &[u8]) -> Result<Vec<Vec<String>>, String> {
    if bytes.starts_with(b"PK") {
        op_export::xlsx::read(bytes).map_err(|e| format!("файл XLSX не читается: {e}"))
    } else {
        let text = String::from_utf8(bytes.to_vec())
            .map_err(|_| "файл CSV должен быть в кодировке UTF-8".to_owned())?;
        Ok(op_export::csv::parse(&text))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_codes_are_unique_and_tokens_round_trip() {
        let t = template();
        let mut codes: Vec<&str> = t.iter().map(|p| p.code.as_str()).collect();
        codes.sort_unstable();
        codes.dedup();
        assert_eq!(codes.len(), t.len());
        for p in &t {
            let token = measure_token(&p.measure);
            assert_eq!(parse_measure(&token).as_ref(), Some(&p.measure), "{token}");
        }
        assert_eq!(
            parse_measure("finish:*:tile"),
            Some(Measure::Finish {
                surface: None,
                kind: Some(MaterialKind::Tile)
            })
        );
        assert_eq!(parse_measure("devices:нет"), None);
    }

    #[test]
    fn prices_survive_csv_and_xlsx() {
        let prices = template();
        let rows = op_export::csv::parse(&prices_csv(&prices));
        let back = import(&[], &rows, "02.11.2026", || "x".into()).unwrap();
        assert_eq!(back.added, prices.len());
        for (a, b) in prices.iter().zip(&back.prices) {
            assert_eq!(
                (&a.code, &a.name, &a.measure, a.kind),
                (&b.code, &b.name, &b.measure, b.kind)
            );
            assert!((a.price - b.price).abs() < 1e-6 && (a.reserve - b.reserve).abs() < 1e-6);
        }
        let book = op_export::xlsx::write(&[Worksheet {
            name: "Прайс".into(),
            tables: vec![prices_table(&prices)],
        }])
        .unwrap();
        let rows = read_table(&book).unwrap();
        let back = import(&[], &rows, "02.11.2026", || "x".into()).unwrap();
        assert_eq!(back.added, prices.len());
    }

    #[test]
    fn import_updates_by_code_and_keeps_history() {
        let prices = template();
        let rows = vec![
            vec!["Код".into(), "Наименование".into(), "Цена".into()],
            vec!["Д-01".into(), "Демонтаж перегородок".into(), "700".into()],
            vec![String::new(), "Новая работа".into(), "1 200,50".into()],
        ];
        let out = import(&prices, &rows, "02.11.2026", || "new".into()).unwrap();
        assert_eq!((out.added, out.updated), (1, 1));
        let d = out.prices.iter().find(|p| p.code == "Д-01").unwrap();
        assert!((d.price - 700.0).abs() < 1e-9);
        assert_eq!(
            d.history,
            vec![PricePoint {
                date: "02.11.2026".into(),
                price: 650.0
            }]
        );
        let new = out.prices.iter().find(|p| p.id == "new").unwrap();
        assert!((new.price - 1200.5).abs() < 1e-9 && new.measure == Measure::Manual);
        assert!(import(&prices, &[vec!["x".into()]], "", String::new).is_err());
    }
}
