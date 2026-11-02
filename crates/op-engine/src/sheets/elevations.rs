//! Развёртки стен: каждая грань помещения — вид изнутри с проёмами и размерами,
//! рядом — схема помещения с буквами стен; итог — площади стен за вычетом проёмов.
//!
//! Развёртки собираются по итоговому виду (вариант, если он выбран, иначе обмер),
//! в масштабе 1:50 или мельче, если самая длинная стена не помещается на лист.

use glam::DVec2;
use op_export::{
    Align, Item, Rgb, Sheet, Text, Viewport,
    frame::{self, TITLE_HEIGHT},
    table::{Cell, Column, THICK, Table},
};
use op_geom::left_normal;
use op_model::OpeningKind;

use super::{
    CAPTION_BAND, GAP, LAYER_DIMENSIONS, LAYER_FURNITURE, LAYER_OPENINGS, LAYER_TABLES,
    LAYER_WALLS, NAME_HEIGHT, Plot, SCALES, SMALL_HEIGHT, Views, flow, new_sheet,
};
use op_geom::tiling;

use crate::{
    finish,
    geometry::DimensionGeometry,
    surfaces::{Face, faces, normalize},
};

/// Название листа.
pub(super) const TITLE: &str = "Развёртки стен";
/// Масштаб развёрток по умолчанию.
const SCALE: u32 = 50;
// Поля вокруг развёртки стены на листе, мм: слева — размер высоты, сверху —
// подпись, снизу — цепочка и общий размер.
const MARGIN_LEFT: f64 = 14.0;
const MARGIN_RIGHT: f64 = 10.0;
const MARGIN_TOP: f64 = 7.0;
const MARGIN_BOTTOM: f64 = 18.0;
/// Схема помещения: сторона квадрата, мм.
const KEY_PLAN: f64 = 44.0;
/// Строка заголовка помещения, мм.
const HEADING: f64 = 8.0;

/// Отделка на грани: полоса высот, материал и раскладка (координаты грани).
#[derive(Debug, Clone)]
struct FaceFinish {
    bottom: f64,
    top: f64,
    name: String,
    /// Куски плитки: контур и признак узкой подрезки.
    pieces: Vec<(Vec<DVec2>, bool)>,
    narrow: usize,
}

/// Помещение с гранями.
#[derive(Debug, Clone)]
pub(super) struct RoomElevation {
    number: usize,
    name: String,
    /// Высота потолка, мм.
    height: f64,
    contour: Vec<DVec2>,
    faces: Vec<Face>,
    /// Отделка каждой грани (по порядку граней).
    finishes: Vec<Vec<FaceFinish>>,
}

impl RoomElevation {
    fn perimeter(&self) -> f64 {
        self.faces.iter().map(|f| f.length).sum::<f64>() / 1000.0
    }

    /// Площадь стен без вычета проёмов, м².
    fn gross(&self) -> f64 {
        self.perimeter() * self.height / 1000.0
    }

    /// Площадь проёмов в пределах высоты потолка, м².
    fn openings(&self) -> f64 {
        self.faces
            .iter()
            .flat_map(|f| &f.openings)
            .map(|o| (o.to - o.from) * (o.top.min(self.height) - o.bottom).max(0.0) / 1e6)
            .sum()
    }
}

/// Развёртки помещений итогового вида в порядке экспликации.
pub(super) fn rooms(views: &Views<'_>) -> Vec<RoomElevation> {
    let (doc, derived, rows) = match &views.variant {
        Some(v) => (&v.doc, &v.derived, views.variant_rooms()),
        None => (&views.survey, &views.survey_derived, views.survey_rooms()),
    };
    rows.iter()
        .enumerate()
        .filter_map(|(i, row)| {
            let region = &derived.regions[*derived.room_regions.get(&row.id)?];
            let contour = normalize(&region.contour);
            let faces = faces(doc, derived, &contour);
            let finishes = face_finishes(doc, derived, &row.id, faces.len());
            (!faces.is_empty()).then(|| RoomElevation {
                number: i + 1,
                name: row.name.clone(),
                height: row.height * 1000.0,
                contour,
                faces,
                finishes,
            })
        })
        .collect()
}

/// Отделка стен помещения по граням: полосы, названия материалов, раскладка.
fn face_finishes(
    doc: &op_model::Document,
    derived: &crate::derived::Derived,
    room: &str,
    count: usize,
) -> Vec<Vec<FaceFinish>> {
    let mut out = vec![Vec::new(); count];
    let Some(geometry) = finish::room_geometry(doc, derived, room) else {
        return out;
    };
    for f in doc
        .finishes
        .iter()
        .filter(|f| f.room == room && f.surface == op_model::Surface::Walls)
    {
        let Some(material) = finish::material(doc, &f.material) else {
            continue;
        };
        for band in finish::wall_bands(doc, f, &geometry) {
            let Some(face) = geometry.faces.get(band.face) else {
                continue;
            };
            let area = finish::band_area(face, &band);
            let layout = finish::spec(&material, f, DVec2::from(f.layout.origin))
                .map(|spec| tiling::layout(&area, &spec, true))
                .filter(|l| !l.approximate)
                .unwrap_or_default();
            if let Some(slot) = out.get_mut(band.face) {
                slot.push(FaceFinish {
                    bottom: band.bottom,
                    top: band.top,
                    name: material.name.clone(),
                    narrow: layout.narrow,
                    pieces: layout
                        .pieces
                        .into_iter()
                        .map(|p| (p.outline, p.narrow))
                        .collect(),
                });
            }
        }
    }
    out
}

/// Таблица площадей стен по помещениям.
pub(super) fn walls_table(rooms: &[RoomElevation]) -> Option<Table> {
    if rooms.is_empty() {
        return None;
    }
    let total = |f: &dyn Fn(&RoomElevation) -> f64| rooms.iter().map(f).sum::<f64>();
    Some(Table {
        title: "Площади стен".into(),
        columns: vec![
            Column::new("№", 12.0, Align::Center),
            Column::new("Помещение", 61.0, Align::Left),
            Column::new("Периметр, м", 22.0, Align::Right),
            Column::new("Высота, м", 22.0, Align::Right),
            Column::new("Стены, м²", 22.0, Align::Right),
            Column::new("Проёмы, м²", 23.0, Align::Right),
            Column::new("Чистая, м²", 23.0, Align::Right),
        ],
        rows: rooms
            .iter()
            .map(|r| {
                vec![
                    Cell::text(r.number.to_string()),
                    Cell::text(r.name.clone()),
                    Cell::number(r.perimeter(), 2),
                    Cell::number(r.height / 1000.0, 2),
                    Cell::number(r.gross(), 2),
                    Cell::number(r.openings(), 2),
                    Cell::number(r.gross() - r.openings(), 2),
                ]
            })
            .collect(),
        footer: Some(vec![
            Cell::text(""),
            Cell::text("Итого"),
            Cell::number(total(&RoomElevation::perimeter), 2),
            Cell::text(""),
            Cell::number(total(&RoomElevation::gross), 2),
            Cell::number(total(&RoomElevation::openings), 2),
            Cell::number(total(&|r| r.gross() - r.openings()), 2),
        ]),
    })
}

/// Размер развёртки стены на листе, мм.
fn face_box(face: &Face, height: f64, k: f64) -> DVec2 {
    DVec2::new(
        face.length / k + MARGIN_LEFT + MARGIN_RIGHT,
        height / k + MARGIN_TOP + MARGIN_BOTTOM,
    )
}

/// Масштаб: 1:50 или ближайший мельче, при котором каждая стена помещается на лист.
fn scale(rooms: &[RoomElevation], area: DVec2) -> u32 {
    SCALES
        .iter()
        .copied()
        .filter(|&s| s >= SCALE)
        .find(|&s| {
            rooms.iter().all(|r| {
                r.faces.iter().all(|f| {
                    let b = face_box(f, r.height, f64::from(s));
                    b.x <= area.x && b.y + HEADING <= area.y
                })
            })
        })
        .unwrap_or(SCALES[SCALES.len() - 1])
}

/// Листы развёрток; таблица площадей стен — после развёрток.
pub(super) fn sheets(rooms: &[RoomElevation], table: Option<Table>, size: DVec2) -> Vec<Sheet> {
    if rooms.is_empty() {
        return Vec::new();
    }
    let (lo, hi) = frame::inner(size);
    let left = lo.x + GAP;
    let right = hi.x - GAP;
    let top = lo.y + CAPTION_BAND;
    let bottom = hi.y - TITLE_HEIGHT - GAP;
    let scale = scale(rooms, DVec2::new(right - left, bottom - top));
    let k = f64::from(scale);

    let mut pages: Vec<Page> = vec![Page::new(size, k, scale, 0)];
    let mut at = DVec2::new(left, top);
    let mut row = 0.0_f64;
    for room in rooms {
        // Помещение начинается с новой строки: заголовок, схема, стены.
        if at.x > left {
            at = DVec2::new(left, at.y + row + GAP);
        }
        let first = room
            .faces
            .first()
            .map_or(KEY_PLAN, |f| face_box(f, room.height, k).y.max(KEY_PLAN));
        if at.y + HEADING + first > bottom {
            pages.push(Page::new(size, k, scale, pages.len()));
            at = DVec2::new(left, top);
        }
        let mut page = pages.len() - 1;
        pages[page].heading(at, &format!("{}. {}", room.number, room.name));
        at.y += HEADING;
        pages[page].key_plan(at, room);
        at.x += KEY_PLAN + GAP;
        row = KEY_PLAN;
        for (n, face) in room.faces.iter().enumerate() {
            let b = face_box(face, room.height, k);
            if at.x + b.x > right {
                at = DVec2::new(left, at.y + row + GAP);
                row = 0.0;
            }
            if at.y + b.y > bottom {
                pages.push(Page::new(size, k, scale, pages.len()));
                page += 1;
                at = DVec2::new(left, top);
                pages[page].heading(at, &format!("{}. {} (продолжение)", room.number, room.name));
                at.y += HEADING;
                row = 0.0;
            }
            pages[page].face(
                at,
                face,
                room.height,
                room.finishes.get(n).map_or(&[], Vec::as_slice),
            );
            at.x += b.x + GAP;
            row = row.max(b.y);
        }
    }
    // Таблица — с новой строки; не поместилась — продолжается на следующем листе.
    let mut tables: Vec<Table> = table.into_iter().collect();
    at.y += row + GAP;
    while !tables.is_empty() {
        let (placed, rest) = flow(std::mem::take(&mut tables), bottom - at.y);
        if placed.is_empty() && (at.y - top).abs() < 1e-9 {
            break;
        }
        let page = pages.len() - 1;
        for t in placed {
            let layer = pages[page].sheet.layer(LAYER_TABLES);
            t.draw(layer, DVec2::new(left, at.y), &mut pages[page].sheet.paper);
            at.y += t.height() + GAP;
        }
        tables = rest;
        if !tables.is_empty() {
            pages.push(Page::new(size, k, scale, pages.len()));
            at = DVec2::new(left, top);
        }
    }
    pages.into_iter().map(Page::finish).collect()
}

/// Лист развёрток в работе: примитивы на бумаге и в модели (мм стены = бумага × k).
struct Page {
    sheet: Sheet,
    model: Vec<Item>,
    k: f64,
}

impl Page {
    fn new(size: DVec2, k: f64, scale: u32, index: usize) -> Self {
        let title = match index {
            0 => TITLE.to_owned(),
            1 => format!("{TITLE} (продолжение)"),
            n => format!("{TITLE} (продолжение {n})"),
        };
        let mut sheet = new_sheet(&title, size);
        let (lo, hi) = frame::inner(size);
        let layer = sheet.layer(LAYER_TABLES);
        sheet.paper.push(Item::text(
            layer,
            Text::centered(
                DVec2::new(lo.x.midpoint(hi.x), lo.y + CAPTION_BAND / 2.0),
                format!("{TITLE} (М 1:{scale})"),
                NAME_HEIGHT,
                Align::Center,
            ),
        ));
        Self {
            sheet,
            model: Vec::new(),
            k,
        }
    }

    fn finish(mut self) -> Sheet {
        let clip = frame::inner(self.sheet.size);
        self.sheet.viewports.push(Viewport {
            items: self.model,
            center: DVec2::ZERO,
            origin: DVec2::ZERO,
            denominator: self.k,
            clip,
        });
        self.sheet
    }

    fn paper_text(&mut self, at: DVec2, text: &str, height: f64, align: Align) {
        let layer = self.sheet.layer(LAYER_TABLES);
        self.sheet
            .paper
            .push(Item::text(layer, Text::centered(at, text, height, align)));
    }

    fn heading(&mut self, at: DVec2, text: &str) {
        self.paper_text(
            at + DVec2::new(0.0, HEADING / 2.0),
            text,
            NAME_HEIGHT,
            Align::Left,
        );
    }

    /// Схема помещения с буквами стен у граней.
    fn key_plan(&mut self, at: DVec2, room: &RoomElevation) {
        let layer = self.sheet.layer(LAYER_WALLS);
        let (mut lo, mut hi) = (DVec2::splat(f64::INFINITY), DVec2::splat(f64::NEG_INFINITY));
        for p in &room.contour {
            lo = lo.min(*p);
            hi = hi.max(*p);
        }
        let inner = KEY_PLAN - 12.0;
        let s = inner / (hi - lo).max_element().max(1.0);
        let center = at + DVec2::splat(KEY_PLAN / 2.0);
        let map = |p: DVec2| center + (p - lo.midpoint(hi)) * s;
        self.sheet.paper.push(Item::polyline(
            layer,
            room.contour.iter().map(|p| map(*p)).collect(),
            true,
            0.35,
            Rgb::BLACK,
        ));
        for f in &room.faces {
            let dir = (f.end - f.start).normalize_or_zero();
            let inward = -left_normal(dir);
            let p = map(f.start.midpoint(f.end)) + inward * 3.0;
            self.paper_text(p, &f.letter, SMALL_HEIGHT, Align::Center);
        }
    }

    /// Развёртка стены: контур, отделка с раскладкой, проёмы, размеры. `at` —
    /// левый верхний угол места на листе. Узкие подрезки — в заголовке стены.
    fn face(&mut self, at: DVec2, face: &Face, height: f64, finishes: &[FaceFinish]) {
        let k = self.k;
        self.paper_text(
            at + DVec2::new(MARGIN_LEFT, MARGIN_TOP / 2.0),
            &format!("Стена {}", face.letter),
            NAME_HEIGHT,
            Align::Left,
        );
        let narrow: usize = finishes.iter().map(|f| f.narrow).sum();
        if narrow > 0 {
            let layer = self.sheet.layer(LAYER_TABLES);
            let mut text = Text::centered(
                at + DVec2::new(MARGIN_LEFT + 22.0, MARGIN_TOP / 2.0),
                format!("узкие подрезки: {narrow}"),
                SMALL_HEIGHT,
                Align::Left,
            );
            text.color = Rgb::hex(0xC2410C);
            self.sheet.paper.push(Item::text(layer, text));
        }
        // Начало координат стены — левый нижний угол (пол), ось Y вниз.
        let floor = (at + DVec2::new(MARGIN_LEFT, MARGIN_TOP + height / k)) * k;
        let point = |x: f64, y: f64| floor + DVec2::new(x, -y);
        let walls = self.sheet.layer(LAYER_WALLS);
        let openings = self.sheet.layer(LAYER_OPENINGS);
        let dims = self.sheet.layer(LAYER_DIMENSIONS);
        let mut plot = Plot {
            sheet: &mut self.sheet,
            items: std::mem::take(&mut self.model),
            k,
        };
        // Отделка: раскладка тонкими линиями (узкие подрезки — красным), граница
        // полосы и название материала у её низа.
        let finish_layer = plot.sheet.layer("Отделка");
        for f in finishes {
            for (piece, narrow) in &f.pieces {
                plot.items.push(Item::polyline(
                    finish_layer,
                    piece.iter().map(|p| point(p.x, p.y)).collect(),
                    true,
                    0.09,
                    if *narrow {
                        Rgb::hex(0xC2410C)
                    } else {
                        Rgb::hex(0x888888)
                    },
                ));
            }
            if f.bottom > 1.0 {
                plot.items.push(Item {
                    layer: finish_layer,
                    shape: op_export::sheet::Shape::Stroke {
                        points: vec![point(0.0, f.bottom), point(face.length, f.bottom)],
                        closed: false,
                        width: 0.25,
                        color: Rgb::BLACK,
                        dash: Some((2.0, 1.0)),
                    },
                });
            }
            if f.top < height - 1.0 {
                plot.items.push(Item {
                    layer: finish_layer,
                    shape: op_export::sheet::Shape::Stroke {
                        points: vec![point(0.0, f.top), point(face.length, f.top)],
                        closed: false,
                        width: 0.25,
                        color: Rgb::BLACK,
                        dash: Some((2.0, 1.0)),
                    },
                });
            }
            if f.top - f.bottom > 4.0 * k {
                plot.text(
                    finish_layer,
                    point(1.5 * k, f.bottom + 2.0 * k),
                    &f.name,
                    SMALL_HEIGHT * 0.8,
                    Align::Left,
                    Rgb::hex(0x333333),
                );
            }
        }
        plot.items.push(Item::polyline(
            walls,
            vec![
                point(0.0, 0.0),
                point(face.length, 0.0),
                point(face.length, height),
                point(0.0, height),
            ],
            true,
            THICK,
            Rgb::BLACK,
        ));
        for o in &face.openings {
            let top = o.top.min(height);
            let (a, b) = (point(o.from, o.bottom), point(o.to, top));
            // Проём закрывает раскладку.
            plot.items.push(Item::fill(
                openings,
                vec![vec![a, DVec2::new(b.x, a.y), b, DVec2::new(a.x, b.y)]],
                Rgb::WHITE,
            ));
            plot.items.push(Item::polyline(
                openings,
                vec![a, DVec2::new(b.x, a.y), b, DVec2::new(a.x, b.y)],
                true,
                0.35,
                Rgb::BLACK,
            ));
            match o.kind {
                // Дверь: треугольник от петель к ручке.
                OpeningKind::Door => {
                    let (hinge, handle) = if o.hinge_left {
                        (o.from, o.to)
                    } else {
                        (o.to, o.from)
                    };
                    let mid = point(handle, (o.bottom + top) / 2.0);
                    for y in [o.bottom, top] {
                        plot.items.push(Item::line(
                            openings,
                            point(hinge, y),
                            mid,
                            0.18,
                            Rgb::BLACK,
                        ));
                    }
                }
                // Окно: рама и средний импост.
                OpeningKind::Window => {
                    let inset = 50.0_f64.min((o.to - o.from) / 4.0);
                    let (c, d) = (
                        point(o.from + inset, o.bottom + inset),
                        point(o.to - inset, top - inset),
                    );
                    plot.items.push(Item::polyline(
                        openings,
                        vec![c, DVec2::new(d.x, c.y), d, DVec2::new(c.x, d.y)],
                        true,
                        0.18,
                        Rgb::BLACK,
                    ));
                    let x = o.from.midpoint(o.to);
                    plot.items.push(Item::line(
                        openings,
                        point(x, o.bottom + inset),
                        point(x, top - inset),
                        0.18,
                        Rgb::BLACK,
                    ));
                }
                OpeningKind::Arch => {}
            }
            // Высоты проёма справа от него: низ (у окна) и верх.
            let mut ys = vec![0.0];
            if o.bottom > 1.0 {
                ys.push(o.bottom);
            }
            ys.push(top);
            for pair in ys.windows(2) {
                plot.dimension(
                    dims,
                    &dimension(
                        point(o.to, pair[0]),
                        point(o.to, pair[1]),
                        DVec2::new(3.0 * k, 0.0),
                    ),
                );
            }
        }
        let furniture = plot.sheet.layer(LAYER_FURNITURE);
        for f in &face.furniture {
            let top = f.top.min(height);
            if top <= f.bottom {
                continue;
            }
            let (a, b) = (point(f.from, f.bottom), point(f.to, top));
            plot.items.push(Item::polyline(
                furniture,
                vec![a, DVec2::new(b.x, a.y), b, DVec2::new(a.x, b.y)],
                true,
                0.25,
                Rgb::BLACK,
            ));
        }
        // Цепочка под стеной: края проёмов, затем общий размер.
        let mut xs = vec![0.0, face.length];
        for o in &face.openings {
            xs.extend([o.from, o.to]);
        }
        xs.sort_by(f64::total_cmp);
        xs.dedup_by(|a, b| (*a - *b).abs() < 1.0);
        let below = |d: f64| DVec2::new(0.0, d * k);
        for pair in xs.windows(2) {
            plot.dimension(
                dims,
                &dimension(point(pair[0], 0.0), point(pair[1], 0.0), below(6.0)),
            );
        }
        if xs.len() > 2 {
            plot.dimension(
                dims,
                &dimension(point(0.0, 0.0), point(face.length, 0.0), below(12.0)),
            );
        }
        // Высота помещения слева.
        plot.dimension(
            dims,
            &dimension(
                point(0.0, 0.0),
                point(0.0, height),
                DVec2::new(-6.0 * k, 0.0),
            ),
        );
        self.model = plot.items;
    }
}

/// Размер между точками `a` и `b`, размерная линия смещена на `offset`.
fn dimension(a: DVec2, b: DVec2, offset: DVec2) -> DimensionGeometry {
    let length = a.distance(b);
    let dir = if length > 1e-9 {
        (b - a) / length
    } else {
        DVec2::X
    };
    DimensionGeometry {
        a,
        b,
        qa: a + offset,
        qb: b + offset,
        dir,
        normal: left_normal(dir),
        length,
    }
}
