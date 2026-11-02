//! Сантехника: каталог точек (выводы, стояки, коллекторы), обозначения, выводы
//! под приборы, прокладка труб, проверки уклонов и отметок, трубы и фитинги.
//!
//! Обозначения нарисованы с нуля по смыслу ГОСТ 21.601 (clean room): системы
//! различаются подписями В1 (холодная вода), Т3 (горячая), К1 (канализация).
//! Вывод ставится, как электроточка, спинкой (−Y) к грани стены.

use std::collections::BTreeMap;

use glam::{DVec2, DVec3};
use op_calc::plumbing as calc;
use op_geom::segment;
use op_model::{Document, Furniture, Pipe, PipeSystem, PipeVertex, PlumbingKind, PlumbingPoint};

use crate::{
    derived::Derived,
    furniture::Mark,
    protocol::{BillRow, PipeReport, PlumbingEntry, PlumbingReport},
};

/// Размер обозначения на плане, мм.
pub const SYMBOL: f64 = 160.0;
/// Отметка разводки водопровода у пола, мм.
pub const WATER_RUN: f64 = 150.0;
/// Конец трубы ближе этого к точке — труба к ней подключена, мм.
const JOIN: f64 = 5.0;

/// Позиция каталога.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlumbingItem {
    /// Идентификатор.
    pub id: &'static str,
    /// Вид.
    pub kind: PlumbingKind,
    /// Система.
    pub system: PipeSystem,
    /// Название.
    pub name: &'static str,
    /// Отметка, мм.
    pub height: f64,
    /// Диаметр, мм.
    pub diameter: f64,
}

const fn p(
    id: &'static str,
    kind: PlumbingKind,
    system: PipeSystem,
    name: &'static str,
    height: f64,
    diameter: f64,
) -> PlumbingItem {
    PlumbingItem {
        id,
        kind,
        system,
        name,
        height,
        diameter,
    }
}

use PipeSystem::{Cold, Drain, Hot};
use PlumbingKind::{Manifold, Outlet, Riser};

/// Каталог.
pub const ITEMS: &[PlumbingItem] = &[
    p("outlet-cold", Outlet, Cold, "Вывод ХВС", 550.0, 20.0),
    p("outlet-hot", Outlet, Hot, "Вывод ГВС", 550.0, 20.0),
    p(
        "outlet-drain-50",
        Outlet,
        Drain,
        "Вывод канализации Ø50",
        500.0,
        50.0,
    ),
    p(
        "outlet-drain-110",
        Outlet,
        Drain,
        "Вывод канализации Ø110",
        200.0,
        110.0,
    ),
    p("riser-cold", Riser, Cold, "Стояк ХВС", 300.0, 32.0),
    p("riser-hot", Riser, Hot, "Стояк ГВС", 300.0, 32.0),
    p(
        "riser-drain",
        Riser,
        Drain,
        "Стояк канализации",
        100.0,
        110.0,
    ),
    p(
        "manifold-cold",
        Manifold,
        Cold,
        "Коллектор ХВС",
        400.0,
        25.0,
    ),
    p("manifold-hot", Manifold, Hot, "Коллектор ГВС", 400.0, 25.0),
];

/// Позиция каталога по идентификатору.
#[must_use]
pub fn item(id: &str) -> Option<&'static PlumbingItem> {
    ITEMS.iter().find(|i| i.id == id)
}

/// Обозначение системы: В1, Т3, К1.
#[must_use]
pub fn code(system: PipeSystem) -> &'static str {
    match system {
        Cold => "В1",
        Hot => "Т3",
        Drain => "К1",
    }
}

/// Название системы.
#[must_use]
pub fn system_name(system: PipeSystem) -> &'static str {
    match system {
        Cold => "Холодное водоснабжение",
        Hot => "Горячее водоснабжение",
        Drain => "Канализация",
    }
}

/// Название точки.
#[must_use]
pub fn point_name(point: &PlumbingPoint) -> String {
    let kind = match point.kind {
        Outlet => "Вывод",
        Riser => "Стояк",
        Manifold => "Коллектор",
    };
    format!("{kind} {} Ø{}", code(point.system), point.diameter)
}

/// Вывод ставится на стену.
#[must_use]
pub fn on_wall(kind: PlumbingKind) -> bool {
    kind == Outlet
}

fn circle(c: DVec2, r: f64) -> Vec<DVec2> {
    (0..20)
        .map(|i| {
            let a = f64::from(i) / 20.0 * std::f64::consts::TAU;
            c + DVec2::new(a.cos(), a.sin()) * r
        })
        .collect()
}

fn mark(points: Vec<DVec2>, closed: bool) -> Mark {
    Mark {
        points,
        closed,
        dashed: false,
    }
}

/// Обозначение в локальных осях: вывод — от грани стены (y = 0) в помещение (+Y),
/// стояк и коллектор — вокруг центра.
#[must_use]
pub fn symbol(kind: PlumbingKind, system: PipeSystem) -> Vec<Mark> {
    let r = SYMBOL / 2.0;
    let v = DVec2::new;
    let mut out = Vec::new();
    match kind {
        Outlet => {
            // Вывод — на грани стены (y = 0): отвод и кружок; у канализации кружок
            // перечёркнут, у ГВС — двойной.
            let c = v(0.0, r);
            out.push(mark(vec![v(0.0, 0.0), c - v(0.0, r * 0.5)], false));
            out.push(mark(circle(c, r * 0.5), true));
            match system {
                Hot => out.push(mark(circle(c, r * 0.25), true)),
                Drain => out.push(mark(vec![c - v(r * 0.5, 0.0), c + v(r * 0.5, 0.0)], false)),
                Cold => {}
            }
        }
        Riser => {
            out.push(mark(circle(DVec2::ZERO, r), true));
            out.push(mark(circle(DVec2::ZERO, r * 0.55), true));
            if system == Drain {
                let s = r * std::f64::consts::FRAC_1_SQRT_2;
                out.push(mark(vec![v(-s, -s), v(s, s)], false));
            }
        }
        Manifold => {
            let (w, h) = (r * 2.0, r * 0.45);
            out.push(mark(vec![v(-w, -h), v(w, -h), v(w, h), v(-w, h)], true));
            for x in [-0.6, -0.2, 0.2, 0.6] {
                out.push(mark(vec![v(x * w, h), v(x * w, h + r * 0.4)], false));
            }
        }
    }
    out
}

/// Точка в локальных осях → точка плана.
#[must_use]
pub fn to_plan(point: &PlumbingPoint, local: DVec2) -> DVec2 {
    let (sin, cos) = point.rotation.to_radians().sin_cos();
    DVec2::new(point.x, point.y)
        + DVec2::new(local.x * cos - local.y * sin, local.x * sin + local.y * cos)
}

/// Обозначение на плане.
#[must_use]
pub fn plan_marks(point: &PlumbingPoint) -> Vec<Mark> {
    symbol(point.kind, point.system)
        .into_iter()
        .map(|m| Mark {
            points: m.points.iter().map(|q| to_plan(point, *q)).collect(),
            ..m
        })
        .collect()
}

/// Попадает ли точка плана в обозначение.
#[must_use]
pub fn contains(point: &PlumbingPoint, q: DVec2, pad: f64) -> bool {
    let (center, reach) = match point.kind {
        Manifold => (DVec2::ZERO, SYMBOL),
        Riser => (DVec2::ZERO, SYMBOL * 0.55),
        Outlet => (DVec2::new(0.0, SYMBOL * 0.35), SYMBOL * 0.55),
    };
    to_plan(point, center).distance(q) <= reach + pad
}

/// Попадает ли точка плана в трубу.
#[must_use]
pub fn pipe_contains(pipe: &Pipe, q: DVec2, pad: f64) -> bool {
    pipe.points
        .windows(2)
        .any(|w| segment::distance(xy(&w[0]), xy(&w[1]), q) <= pipe.diameter / 2.0 + pad)
}

/// Точка плана вершины.
#[must_use]
pub fn xy(v: &PipeVertex) -> DVec2 {
    DVec2::new(v.x, v.y)
}

fn xyz(v: &PipeVertex) -> DVec3 {
    DVec3::new(v.x, v.y, v.z)
}

/// Длина трубы по оси с подъёмами, мм.
#[must_use]
pub fn length(pipe: &Pipe) -> f64 {
    pipe.points
        .windows(2)
        .map(|w| xyz(&w[0]).distance(xyz(&w[1])))
        .sum()
}

/// Каталог для UI.
#[must_use]
pub fn catalog_entries() -> Vec<PlumbingEntry> {
    ITEMS
        .iter()
        .map(|i| {
            let k = 90.0 / (SYMBOL * if i.kind == Manifold { 2.4 } else { 1.2 });
            let icon = symbol(i.kind, i.system)
                .iter()
                .map(|m| {
                    let mut s = String::new();
                    for (j, q) in m.points.iter().enumerate() {
                        let q = DVec2::splat(50.0) + *q * k;
                        s.push_str(&format!(
                            "{}{:.1} {:.1}",
                            if j == 0 { "M" } else { "L" },
                            q.x,
                            q.y
                        ));
                    }
                    if m.closed {
                        s.push('Z');
                    }
                    s
                })
                .collect();
            PlumbingEntry {
                id: i.id.into(),
                kind: i.kind,
                system: i.system,
                name: i.name.into(),
                group: match i.kind {
                    Outlet => "Выводы",
                    Riser => "Стояки",
                    Manifold => "Коллекторы",
                }
                .into(),
                height: i.height,
                diameter: i.diameter,
                on_wall: on_wall(i.kind),
                icon,
            }
        })
        .collect()
}

/// Вывод прибора: система, смещение вдоль спинки, отметка, диаметр.
struct FixtureOutlet {
    system: PipeSystem,
    dx: f64,
    height: f64,
    diameter: f64,
}

const fn o(system: PipeSystem, dx: f64, height: f64, diameter: f64) -> FixtureOutlet {
    FixtureOutlet {
        system,
        dx,
        height,
        diameter,
    }
}

/// Выводы под прибор каталога мебели. Горячая вода — слева, холодная — справа,
/// если стоять лицом к прибору. Отметки — типовые для бытовой сантехники.
fn fixture_outlets(f: &Furniture) -> Vec<FixtureOutlet> {
    match f.item.as_str() {
        "toilet" => vec![o(Cold, 150.0, 200.0, 20.0), o(Drain, 0.0, 200.0, 110.0)],
        "washbasin" | "kitchen-sink" => vec![
            o(Hot, -75.0, 550.0, 20.0),
            o(Cold, 75.0, 550.0, 20.0),
            o(Drain, 0.0, 500.0, 50.0),
        ],
        "bathtub" => {
            // Слив — у торца ванны со стороны смесителя.
            let end = -f.width / 2.0 + 250.0;
            vec![
                o(Hot, end - 75.0, 800.0, 20.0),
                o(Cold, end + 75.0, 800.0, 20.0),
                o(Drain, end, 150.0, 50.0),
            ]
        }
        "shower" => vec![
            o(Hot, -75.0, 1000.0, 20.0),
            o(Cold, 75.0, 1000.0, 20.0),
            o(Drain, 0.0, 100.0, 50.0),
        ],
        "washer" => vec![o(Cold, 100.0, 600.0, 20.0), o(Drain, -100.0, 600.0, 50.0)],
        _ => Vec::new(),
    }
}

/// Прибор: у позиции каталога есть выводы.
#[must_use]
pub fn is_fixture(f: &Furniture) -> bool {
    !fixture_outlets(f).is_empty()
}

/// Выводы под приборы, у которых ещё нет выводов.
#[must_use]
pub fn outlets_for_fixtures(
    doc: &Document,
    derived: &Derived,
    mut new_id: impl FnMut() -> String,
) -> Vec<PlumbingPoint> {
    let mut out = Vec::new();
    for f in doc
        .furniture
        .iter()
        .filter(|f| derived.visible.contains(&f.layer))
    {
        if doc
            .plumbing
            .iter()
            .any(|p| p.fixture.as_deref() == Some(&f.id))
        {
            continue;
        }
        for spec in fixture_outlets(f) {
            let at = crate::furniture::to_plan(f, DVec2::new(spec.dx, -f.depth / 2.0));
            out.push(PlumbingPoint {
                level: None,
                id: new_id(),
                layer: op_model::LayerKind::Plumbing.id().into(),
                variant: None,
                kind: Outlet,
                system: spec.system,
                x: at.x,
                y: at.y,
                rotation: f.rotation,
                height: spec.height,
                diameter: spec.diameter,
                label: f.name.clone(),
                fixture: Some(f.id.clone()),
            });
        }
    }
    out
}

/// Сколько стен пересекает отрезок (по осям стен).
fn crossings(doc: &Document, derived: &Derived, a: DVec2, b: DVec2) -> usize {
    doc.walls
        .iter()
        .filter_map(|w| derived.wall_axis(w))
        .filter(|(p, q)| segment::segment_intersection(a, b, *p, *q).is_some())
        .count()
}

/// Цель трубы от вывода: коллектор своей системы (для воды), иначе ближайший стояк.
fn target<'a>(points: &'a [&'a PlumbingPoint], from: &PlumbingPoint) -> Option<&'a PlumbingPoint> {
    let at = DVec2::new(from.x, from.y);
    let nearest = |kind: PlumbingKind| {
        points
            .iter()
            .filter(|p| p.kind == kind && p.system == from.system)
            .min_by(|a, b| {
                let da = DVec2::new(a.x, a.y).distance(at);
                let db = DVec2::new(b.x, b.y).distance(at);
                da.total_cmp(&db)
            })
            .copied()
    };
    if from.system.is_water()
        && let Some(m) = nearest(Manifold)
    {
        return Some(m);
    }
    nearest(Riser)
}

/// Трасса от вывода к цели: два участка по осям (угол выбирается с меньшим числом
/// пересечений стен). Водопровод опускается к разводке у пола и поднимается к цели,
/// канализация идёт с нормальным уклоном от отметки вывода.
#[must_use]
pub fn route(
    doc: &Document,
    derived: &Derived,
    from: &PlumbingPoint,
    to: &PlumbingPoint,
) -> Vec<PipeVertex> {
    let a = DVec2::new(from.x, from.y);
    let b = DVec2::new(to.x, to.y);
    let corners = [DVec2::new(b.x, a.y), DVec2::new(a.x, b.y)];
    let cost = |c: DVec2| crossings(doc, derived, a, c) + crossings(doc, derived, c, b);
    let corner = if cost(corners[1]) < cost(corners[0]) {
        corners[1]
    } else {
        corners[0]
    };
    let plan: Vec<DVec2> = [a, corner, b]
        .into_iter()
        .fold(Vec::new(), |mut acc: Vec<DVec2>, q| {
            if acc.last().is_none_or(|l| l.distance(q) > 1.0) {
                acc.push(q);
            }
            acc
        });
    let v = |q: DVec2, z: f64| PipeVertex { x: q.x, y: q.y, z };
    if from.system.is_water() {
        let mut out = vec![v(a, from.height)];
        if (from.height - WATER_RUN).abs() > 1.0 {
            out.push(v(a, WATER_RUN));
        }
        out.extend(plan.iter().skip(1).map(|q| v(*q, WATER_RUN)));
        if (to.height - WATER_RUN).abs() > 1.0 {
            out.push(v(b, to.height));
        }
        out
    } else {
        let (slope, _) = calc::slopes(from.diameter);
        let mut z = from.height;
        let mut out = vec![v(a, z)];
        for w in plan.windows(2) {
            z -= w[0].distance(w[1]) * slope;
            out.push(v(w[1], (z * 10.0).round() / 10.0));
        }
        out
    }
}

/// Автоматические трубы от всех выводов к стоякам и коллекторам.
#[must_use]
pub fn route_all(
    doc: &Document,
    derived: &Derived,
    mut new_id: impl FnMut() -> String,
) -> Vec<Pipe> {
    let shown: Vec<&PlumbingPoint> = doc
        .plumbing
        .iter()
        .filter(|p| derived.visible.contains(&p.layer))
        .collect();
    let mut out = Vec::new();
    for from in shown.iter().filter(|p| p.kind == Outlet) {
        // Вывод с ручной трубой не трогаем.
        let manual = doc
            .pipes
            .iter()
            .any(|pipe| !pipe.auto && starts_at(pipe, from));
        if manual {
            continue;
        }
        let Some(to) = target(&shown, from) else {
            continue;
        };
        out.push(Pipe {
            level: None,
            id: new_id(),
            layer: op_model::LayerKind::Plumbing.id().into(),
            variant: None,
            system: from.system,
            diameter: from.diameter,
            points: route(doc, derived, from, to),
            auto: true,
        });
    }
    out
}

fn near(v: &PipeVertex, p: &PlumbingPoint) -> bool {
    xy(v).distance(DVec2::new(p.x, p.y)) <= JOIN
}

fn starts_at(pipe: &Pipe, point: &PlumbingPoint) -> bool {
    pipe.system == point.system
        && (pipe.points.first().is_some_and(|v| near(v, point))
            || pipe.points.last().is_some_and(|v| near(v, point)))
}

/// Вершины трубы для новой ручной трубы: вода — на отметке разводки, канализация
/// — с нормальным уклоном от первой вершины (отметка вывода, если начали с него).
#[must_use]
pub fn manual_vertices(
    doc: &Document,
    system: PipeSystem,
    diameter: f64,
    plan: &[DVec2],
) -> Vec<PipeVertex> {
    let at = |q: DVec2| {
        doc.plumbing
            .iter()
            .find(|p| p.system == system && DVec2::new(p.x, p.y).distance(q) <= JOIN)
    };
    let start = plan
        .first()
        .and_then(|q| at(*q))
        .map_or(if system.is_water() { WATER_RUN } else { 300.0 }, |p| {
            p.height
        });
    let mut z = start;
    let (slope, _) = calc::slopes(diameter);
    plan.iter()
        .enumerate()
        .map(|(i, q)| {
            if i > 0 && !system.is_water() {
                z -= plan[i - 1].distance(*q) * slope;
            }
            PipeVertex {
                x: q.x,
                y: q.y,
                z: if system.is_water() { start } else { z },
            }
        })
        .collect()
}

/// Точка, к которой привязывается вершина трубы: центр сантехнической точки рядом.
#[must_use]
pub fn snap_point(doc: &Document, derived: &Derived, q: DVec2, radius: f64) -> Option<DVec2> {
    doc.plumbing
        .iter()
        .filter(|p| derived.visible.contains(&p.layer))
        .map(|p| DVec2::new(p.x, p.y))
        .filter(|c| c.distance(q) <= radius)
        .min_by(|a, b| a.distance(q).total_cmp(&b.distance(q)))
}

fn percent(v: f64) -> String {
    format!("{:.1}", v * 100.0).replace('.', ",")
}

fn warning(issue: calc::Issue) -> String {
    match issue {
        calc::Issue::FlatSlope(s, min) => format!(
            "уклон {} % меньше наименьшего {} %",
            percent(s),
            percent(min)
        ),
        calc::Issue::SteepSlope(s) => format!(
            "уклон {} % больше наибольшего {} %",
            percent(s),
            percent(calc::MAX_SLOPE)
        ),
        calc::Issue::Uphill => "труба поднимается по ходу стока".into(),
        calc::Issue::BelowInlet(mm) => format!(
            "конец трубы на {mm:.0} мм ниже врезки в стояк: поднимите вывод или сократите трассу"
        ),
    }
}

/// Счётчик строк спецификации по названию.
#[derive(Default)]
struct Bill {
    rows: BTreeMap<(u8, String), (String, f64, String)>,
}

impl Bill {
    fn add(&mut self, order: u8, name: String, unit: &str, amount: f64) {
        let row = self
            .rows
            .entry((order, name))
            .or_insert_with(|| (unit.to_owned(), 0.0, String::new()));
        row.1 += amount;
    }

    fn finish(self) -> Vec<BillRow> {
        self.rows
            .into_iter()
            .map(|((_, name), (unit, amount, note))| BillRow {
                name,
                unit,
                amount,
                note,
            })
            .collect()
    }
}

fn pipe_name(system: PipeSystem, d: f64) -> String {
    match system {
        Cold => format!("Труба PP-R PN20 Ø{d}"),
        Hot => format!("Труба PP-R армированная PN25 Ø{d}"),
        Drain => format!("Труба канализационная ПП Ø{d}"),
    }
}

/// Угол между участками ломаной, градусы.
fn turn(a: DVec3, b: DVec3, c: DVec3) -> f64 {
    let (u, v) = ((b - a).normalize_or_zero(), (c - b).normalize_or_zero());
    u.dot(v).clamp(-1.0, 1.0).acos().to_degrees()
}

/// Расчёт сантехники вида: трубы с проверками, трубы и фитинги, общие предупреждения.
#[must_use]
pub fn report(doc: &Document, derived: &Derived) -> PlumbingReport {
    let points: Vec<&PlumbingPoint> = doc
        .plumbing
        .iter()
        .filter(|p| derived.visible.contains(&p.layer))
        .collect();
    let pipes: Vec<&Pipe> = doc
        .pipes
        .iter()
        .filter(|p| derived.visible.contains(&p.layer))
        .collect();
    let at_end = |pipe: &Pipe, last: bool| {
        let v = if last {
            pipe.points.last()
        } else {
            pipe.points.first()
        };
        v.and_then(|v| {
            points
                .iter()
                .find(|p| p.system == pipe.system && near(v, p))
                .copied()
        })
    };
    let mut bill = Bill::default();
    let mut manifold_outputs: BTreeMap<String, u32> = BTreeMap::new();
    let mut lengths: BTreeMap<(u8, String), (bool, f64)> = BTreeMap::new();
    let reports = pipes
        .iter()
        .map(|pipe| {
            let water = pipe.system.is_water();
            let start = at_end(pipe, false);
            let end = at_end(pipe, true);
            let len = length(pipe);
            let order = match pipe.system {
                Cold => 0,
                Hot => 1,
                Drain => 2,
            };
            let entry = lengths
                .entry((order, pipe_name(pipe.system, pipe.diameter)))
                .or_insert((water, 0.0));
            entry.1 += len / 1000.0;
            // Фитинги на поворотах.
            let v: Vec<DVec3> = pipe.points.iter().map(xyz).collect();
            for w in v.windows(3) {
                let (e90, e45) = calc::bends(turn(w[0], w[1], w[2]), water);
                let kind = if water {
                    "PP-R"
                } else {
                    "канализационный"
                };
                if e90 > 0 {
                    bill.add(
                        order + 10,
                        format!("Отвод 90° {kind} Ø{}", pipe.diameter),
                        "шт.",
                        f64::from(e90),
                    );
                }
                if e45 > 0 {
                    bill.add(
                        order + 10,
                        format!("Отвод 45° {kind} Ø{}", pipe.diameter),
                        "шт.",
                        f64::from(e45),
                    );
                }
            }
            if water {
                let joints = calc::sticks(len / 1000.0, true).saturating_sub(1);
                if joints > 0 {
                    bill.add(
                        order + 20,
                        format!("Муфта PP-R Ø{}", pipe.diameter),
                        "шт.",
                        f64::from(joints),
                    );
                }
                if start.is_some_and(|p| p.kind == Outlet) {
                    bill.add(
                        order + 30,
                        format!("Водорозетка Ø{}×1/2″", pipe.diameter),
                        "шт.",
                        1.0,
                    );
                }
                match end {
                    Some(t) if t.kind == Manifold => {
                        *manifold_outputs.entry(t.id.clone()).or_default() += 1;
                    }
                    Some(t) if t.kind == Riser => {
                        bill.add(
                            order + 30,
                            format!("Тройник PP-R Ø{}×{}", t.diameter, pipe.diameter),
                            "шт.",
                            1.0,
                        );
                        bill.add(
                            order + 30,
                            format!("Кран шаровой Ø{}", pipe.diameter),
                            "шт.",
                            1.0,
                        );
                    }
                    _ => {}
                }
            } else if let Some(t) = end.filter(|t| t.kind == Riser) {
                bill.add(
                    order + 30,
                    format!("Тройник канализационный {}×{}", t.diameter, pipe.diameter),
                    "шт.",
                    1.0,
                );
            }
            // Проверки канализации.
            let mut warnings = Vec::new();
            let mut slope = None;
            if !water {
                let runs: Vec<calc::Run> = pipe
                    .points
                    .windows(2)
                    .map(|w| calc::Run {
                        length: xy(&w[0]).distance(xy(&w[1])),
                        fall: w[0].z - w[1].z,
                    })
                    .collect();
                slope = runs
                    .iter()
                    .filter(|r| r.length > 1.0)
                    .map(|r| r.fall / r.length)
                    .reduce(f64::min);
                let last = pipe.points.last().map_or(0.0, |v| v.z);
                let inlet = end.filter(|t| t.kind == Riser).map(|t| t.height);
                warnings.extend(
                    calc::check_drain(pipe.diameter, &runs, last, inlet)
                        .into_iter()
                        .map(warning),
                );
            }
            if end.is_none() && start.is_some_and(|p| p.kind == Outlet) {
                warnings.push("труба никуда не подключена".into());
            }
            PipeReport {
                id: pipe.id.clone(),
                system: pipe.system,
                diameter: pipe.diameter,
                length: len / 1000.0,
                slope,
                label: start.map(|p| p.label.clone()).unwrap_or_default(),
                warnings,
            }
        })
        .collect();
    let mut rows: Vec<BillRow> = lengths
        .into_iter()
        .map(|((_, name), (water, len))| {
            let with_reserve = (len * (1.0 + calc::PIPE_RESERVE) * 10.0).ceil() / 10.0;
            BillRow {
                name,
                unit: "м".into(),
                amount: with_reserve,
                note: format!(
                    "{} хл. по {} м",
                    calc::sticks(len, water),
                    calc::stick(water)
                ),
            }
        })
        .collect();
    for (id, outputs) in &manifold_outputs {
        if let Some(m) = points.iter().find(|p| &p.id == id) {
            bill.add(
                40,
                format!(
                    "Коллектор {} на {outputs} вых. Ø{}",
                    code(m.system),
                    m.diameter
                ),
                "шт.",
                1.0,
            );
        }
    }
    rows.extend(bill.finish());

    let unconnected = points
        .iter()
        .filter(|p| p.kind == Outlet)
        .filter(|p| !pipes.iter().any(|pipe| starts_at(pipe, p)))
        .count();
    let mut warnings = Vec::new();
    for system in PipeSystem::ALL {
        let outlets = points
            .iter()
            .any(|p| p.kind == Outlet && p.system == system);
        let source = points
            .iter()
            .any(|p| p.kind != Outlet && p.system == system);
        if outlets && !source {
            warnings.push(format!(
                "{}: нет стояка{} — трубы не проложить",
                system_name(system),
                if system.is_water() {
                    " или коллектора"
                } else {
                    ""
                }
            ));
        }
    }
    // Ревизия стояка канализации должна быть доступна.
    for riser in points
        .iter()
        .filter(|p| p.kind == Riser && p.system == Drain)
    {
        let c = DVec2::new(riser.x, riser.y);
        if let Some(f) = doc.furniture.iter().find(|f| {
            derived.visible.contains(&f.layer)
                && f.elevation < 1000.0
                && crate::furniture::contains(f, c, 0.0)
        }) {
            warnings.push(format!(
                "Стояк К1 закрыт предметом «{}»: оставьте доступ к ревизии",
                f.name
            ));
        }
    }
    PlumbingReport {
        pipes: reports,
        bill: rows,
        unconnected: u32::try_from(unconnected).unwrap_or(u32::MAX),
        warnings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_has_symbols_and_unique_ids() {
        let mut ids: Vec<&str> = ITEMS.iter().map(|i| i.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), ITEMS.len());
        assert!(catalog_entries().iter().all(|e| e.icon.starts_with('M')));
        assert!(on_wall(Outlet) && !on_wall(Riser));
    }
}
