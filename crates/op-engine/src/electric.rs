//! Электрика и освещение: каталог точек, условные обозначения, установка на
//! стену, группы щита, трассы кабеля и расчёт (нагрузки, токи, падение
//! напряжения, состав щита, освещённость помещений).
//!
//! Обозначения нарисованы с нуля по смыслу ГОСТ 21.210 (clean room). Точка
//! ставится, как мебель, по локальным осям: на стене — спинкой (−Y) к грани.

use std::collections::HashSet;

use glam::DVec2;
use op_calc::{electrical, lighting};
use op_geom::left_normal;
use op_model::{Circuit, CircuitKind, Device, DeviceKind, Document};

use crate::{
    derived::Derived,
    furniture::Mark,
    protocol::{
        CircuitReport, DeviceEntry, ElectricalReport, PanelItemInfo, PanelReport, RoomLighting,
    },
};

/// Размер обозначения на плане, мм (около 4 мм на листе 1:50).
pub const SYMBOL: f64 = 200.0;
/// Отступ кабельной трассы под потолком, мм.
const CEILING_RUN: f64 = 150.0;
/// Запас кабеля на разделку и неучтённые повороты.
const CABLE_RESERVE: f64 = 1.1;

/// Позиция каталога: вид, название, раздел и значения по умолчанию.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DeviceItem {
    /// Вид.
    pub kind: DeviceKind,
    /// Название.
    pub name: &'static str,
    /// Раздел.
    pub group: &'static str,
    /// Высота установки, мм (у потолочных — высота потолка).
    pub height: f64,
    /// Мощность, Вт.
    pub power: f64,
    /// Световой поток, лм.
    pub flux: f64,
}

const fn d(
    kind: DeviceKind,
    name: &'static str,
    group: &'static str,
    height: f64,
    power: f64,
    flux: f64,
) -> DeviceItem {
    DeviceItem {
        kind,
        name,
        group,
        height,
        power,
        flux,
    }
}

const SOCKETS: &str = "Розетки";
const OUTLETS: &str = "Выводы";
const SWITCHES: &str = "Выключатели";
const LIGHTS: &str = "Светильники";
const PANEL: &str = "Щит и коробки";

/// Каталог электроточек.
pub const DEVICES: &[DeviceItem] = &[
    d(DeviceKind::Socket, "Розетка", SOCKETS, 300.0, 300.0, 0.0),
    d(
        DeviceKind::SocketDouble,
        "Розетка двойная",
        SOCKETS,
        300.0,
        600.0,
        0.0,
    ),
    d(
        DeviceKind::SocketWaterproof,
        "Розетка IP44",
        SOCKETS,
        1000.0,
        300.0,
        0.0,
    ),
    d(
        DeviceKind::SocketPower,
        "Розетка 380 В",
        SOCKETS,
        600.0,
        7000.0,
        0.0,
    ),
    d(
        DeviceKind::SocketData,
        "Розетка компьютерная",
        SOCKETS,
        300.0,
        0.0,
        0.0,
    ),
    d(
        DeviceKind::SocketTv,
        "Розетка телевизионная",
        SOCKETS,
        300.0,
        0.0,
        0.0,
    ),
    d(
        DeviceKind::CableOutlet,
        "Вывод кабеля",
        OUTLETS,
        600.0,
        2000.0,
        0.0,
    ),
    d(
        DeviceKind::FloorOutlet,
        "Напольный лючок",
        OUTLETS,
        0.0,
        300.0,
        0.0,
    ),
    d(
        DeviceKind::CeilingOutlet,
        "Вывод на потолке",
        OUTLETS,
        0.0,
        0.0,
        0.0,
    ),
    d(
        DeviceKind::Switch1,
        "Выключатель одноклавишный",
        SWITCHES,
        900.0,
        0.0,
        0.0,
    ),
    d(
        DeviceKind::Switch2,
        "Выключатель двухклавишный",
        SWITCHES,
        900.0,
        0.0,
        0.0,
    ),
    d(
        DeviceKind::Switch3,
        "Выключатель трёхклавишный",
        SWITCHES,
        900.0,
        0.0,
        0.0,
    ),
    d(
        DeviceKind::PassSwitch,
        "Выключатель проходной",
        SWITCHES,
        900.0,
        0.0,
        0.0,
    ),
    d(
        DeviceKind::CrossSwitch,
        "Выключатель перекрёстный",
        SWITCHES,
        900.0,
        0.0,
        0.0,
    ),
    d(
        DeviceKind::CeilingLight,
        "Светильник потолочный",
        LIGHTS,
        0.0,
        40.0,
        3500.0,
    ),
    d(
        DeviceKind::Pendant,
        "Светильник подвесной",
        LIGHTS,
        0.0,
        20.0,
        1800.0,
    ),
    d(
        DeviceKind::Spot,
        "Светильник точечный",
        LIGHTS,
        0.0,
        7.0,
        550.0,
    ),
    d(DeviceKind::WallLight, "Бра", LIGHTS, 1800.0, 10.0, 700.0),
    d(
        DeviceKind::LedStrip,
        "Светодиодная лента (1 м)",
        LIGHTS,
        0.0,
        14.0,
        1000.0,
    ),
    d(
        DeviceKind::JunctionBox,
        "Распределительная коробка",
        PANEL,
        2500.0,
        0.0,
        0.0,
    ),
    d(DeviceKind::Panel, "Электрощит", PANEL, 1700.0, 0.0, 0.0),
];

/// Позиция каталога по виду.
#[must_use]
pub fn item(kind: DeviceKind) -> Option<&'static DeviceItem> {
    DEVICES.iter().find(|i| i.kind == kind)
}

/// Точка ставится на стену.
#[must_use]
pub fn on_wall(kind: DeviceKind) -> bool {
    !matches!(
        kind,
        DeviceKind::FloorOutlet
            | DeviceKind::CeilingOutlet
            | DeviceKind::CeilingLight
            | DeviceKind::Pendant
            | DeviceKind::Spot
            | DeviceKind::LedStrip
    )
}

/// Розетка или вывод (лист «Розетки»).
#[must_use]
pub fn is_outlet(kind: DeviceKind) -> bool {
    matches!(
        kind,
        DeviceKind::Socket
            | DeviceKind::SocketDouble
            | DeviceKind::SocketWaterproof
            | DeviceKind::SocketPower
            | DeviceKind::SocketData
            | DeviceKind::SocketTv
            | DeviceKind::CableOutlet
            | DeviceKind::FloorOutlet
            | DeviceKind::CeilingOutlet
    )
}

/// Выключатель.
#[must_use]
pub fn is_switch(kind: DeviceKind) -> bool {
    kind.keys() > 0
}

/// Название вида.
#[must_use]
pub fn kind_name(kind: DeviceKind) -> &'static str {
    item(kind).map_or("Электроточка", |i| i.name)
}

fn circle(c: DVec2, r: f64) -> Vec<DVec2> {
    (0..24)
        .map(|i| {
            let a = f64::from(i) / 24.0 * std::f64::consts::TAU;
            c + DVec2::new(a.cos(), a.sin()) * r
        })
        .collect()
}

/// Полуокружность выпуклостью в помещение (+Y), концами на стене.
fn arc_up(c: DVec2, r: f64) -> Vec<DVec2> {
    (0..=12)
        .map(|i| {
            let a = f64::from(i) / 12.0 * std::f64::consts::PI;
            c + DVec2::new(-a.cos(), a.sin()) * r
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

/// Условное обозначение в локальных осях (центр в нуле, стена на −Y).
#[must_use]
pub fn symbol(kind: DeviceKind) -> Vec<Mark> {
    let r = SYMBOL / 2.0;
    let p = DVec2::new;
    let back = -r;
    let socket = |out: &mut Vec<Mark>, x: f64| {
        out.push(mark(arc_up(p(x, back), r * 0.7), false));
        out.push(mark(
            vec![p(x - r * 0.7, back), p(x + r * 0.7, back)],
            false,
        ));
    };
    let mut out = Vec::new();
    match kind {
        DeviceKind::Socket | DeviceKind::SocketData | DeviceKind::SocketTv => {
            socket(&mut out, 0.0);
            out.push(mark(
                vec![p(0.0, back + r * 0.7), p(0.0, back + r * 1.3)],
                false,
            ));
            if kind == DeviceKind::SocketData {
                out.push(mark(
                    vec![
                        p(-r * 0.3, back + r * 1.3),
                        p(r * 0.3, back + r * 1.3),
                        p(0.0, back + r * 1.8),
                    ],
                    true,
                ));
            }
            if kind == DeviceKind::SocketTv {
                out.push(mark(circle(p(0.0, back + r * 1.5), r * 0.25), true));
            }
        }
        DeviceKind::SocketDouble => {
            socket(&mut out, -r * 0.55);
            socket(&mut out, r * 0.55);
        }
        DeviceKind::SocketWaterproof => {
            socket(&mut out, 0.0);
            // Закрашенная половина — защищённое исполнение.
            out.push(mark(
                vec![p(-r * 0.7, back), p(0.0, back + r * 0.7), p(r * 0.7, back)],
                true,
            ));
        }
        DeviceKind::SocketPower => {
            socket(&mut out, 0.0);
            for x in [-0.4, 0.0, 0.4] {
                out.push(mark(
                    vec![p(x * r, back + r * 0.7), p(x * r, back + r * 1.3)],
                    false,
                ));
            }
        }
        DeviceKind::CableOutlet => {
            out.push(mark(vec![p(0.0, back), p(0.0, back + r * 1.4)], false));
            out.push(mark(
                vec![
                    p(-r * 0.35, back + r * 1.0),
                    p(0.0, back + r * 1.4),
                    p(r * 0.35, back + r * 1.0),
                ],
                false,
            ));
        }
        DeviceKind::FloorOutlet | DeviceKind::CeilingOutlet => {
            let s = r * 0.7;
            out.push(mark(vec![p(-s, -s), p(s, -s), p(s, s), p(-s, s)], true));
            out.push(mark(vec![p(-s, -s), p(s, s)], false));
            out.push(mark(vec![p(s, -s), p(-s, s)], false));
        }
        DeviceKind::Switch1
        | DeviceKind::Switch2
        | DeviceKind::Switch3
        | DeviceKind::PassSwitch
        | DeviceKind::CrossSwitch => {
            let c = p(0.0, back + r * 0.45);
            out.push(mark(circle(c, r * 0.4), true));
            let dir = DVec2::new(1.0, 1.0).normalize();
            let lever = |out: &mut Vec<Mark>, d: DVec2, offset: f64| {
                let start = c + d * r * 0.4;
                let end = start + d * r * 0.9;
                out.push(mark(vec![start, end], false));
                let n = DVec2::new(-d.y, d.x) * offset;
                out.push(mark(vec![end, end + n], false));
            };
            match kind {
                DeviceKind::Switch1 => lever(&mut out, dir, r * 0.3),
                DeviceKind::Switch2 => {
                    lever(&mut out, dir, r * 0.3);
                    lever(&mut out, dir, -r * 0.3);
                }
                DeviceKind::Switch3 => {
                    lever(&mut out, dir, r * 0.3);
                    lever(&mut out, dir, -r * 0.3);
                    lever(&mut out, DVec2::new(0.0, 1.0), r * 0.3);
                }
                DeviceKind::PassSwitch => {
                    lever(&mut out, dir, r * 0.3);
                    lever(&mut out, DVec2::new(-dir.x, dir.y), -r * 0.3);
                }
                _ => {
                    for d in [dir, DVec2::new(-dir.x, dir.y)] {
                        lever(&mut out, d, r * 0.3);
                        lever(&mut out, d, -r * 0.3);
                    }
                }
            }
        }
        DeviceKind::CeilingLight => {
            out.push(mark(circle(DVec2::ZERO, r), true));
            let s = r * std::f64::consts::FRAC_1_SQRT_2;
            out.push(mark(vec![p(-s, -s), p(s, s)], false));
            out.push(mark(vec![p(s, -s), p(-s, s)], false));
        }
        DeviceKind::Pendant => {
            out.push(mark(circle(DVec2::ZERO, r), true));
            out.push(mark(circle(DVec2::ZERO, r * 0.35), true));
        }
        DeviceKind::Spot => {
            out.push(mark(circle(DVec2::ZERO, r * 0.5), true));
            out.push(mark(vec![p(-r * 0.8, 0.0), p(r * 0.8, 0.0)], false));
            out.push(mark(vec![p(0.0, -r * 0.8), p(0.0, r * 0.8)], false));
        }
        DeviceKind::WallLight => {
            out.push(mark(arc_up(p(0.0, back), r * 0.8), false));
            out.push(mark(vec![p(-r * 0.8, back), p(r * 0.8, back)], false));
            out.push(mark(vec![p(0.0, back), p(0.0, back + r * 0.8)], false));
        }
        DeviceKind::LedStrip => {
            out.push(Mark {
                points: vec![
                    p(-500.0, -20.0),
                    p(500.0, -20.0),
                    p(500.0, 20.0),
                    p(-500.0, 20.0),
                ],
                closed: true,
                dashed: true,
            });
        }
        DeviceKind::JunctionBox => out.push(mark(circle(p(0.0, back + r * 0.5), r * 0.5), true)),
        DeviceKind::Panel => {
            let (w, h) = (r * 2.4, r * 0.9);
            out.push(mark(
                vec![p(-w, back), p(w, back), p(w, back + h), p(-w, back + h)],
                true,
            ));
            out.push(mark(vec![p(-w, back + h), p(w, back)], false));
        }
    }
    out
}

/// Точка в локальных осях → точка плана.
#[must_use]
pub fn to_plan(device: &Device, local: DVec2) -> DVec2 {
    let (sin, cos) = device.rotation.to_radians().sin_cos();
    DVec2::new(device.x, device.y)
        + DVec2::new(local.x * cos - local.y * sin, local.x * sin + local.y * cos)
}

/// Обозначение на плане.
#[must_use]
pub fn plan_marks(device: &Device) -> Vec<Mark> {
    symbol(device.kind)
        .into_iter()
        .map(|m| Mark {
            points: m.points.iter().map(|p| to_plan(device, *p)).collect(),
            ..m
        })
        .collect()
}

/// Попадает ли точка в обозначение.
#[must_use]
pub fn contains(device: &Device, p: DVec2, pad: f64) -> bool {
    let reach = if device.kind == DeviceKind::LedStrip {
        500.0
    } else {
        SYMBOL * 0.6
    };
    let center = to_plan(
        device,
        DVec2::new(
            0.0,
            if on_wall(device.kind) {
                -SYMBOL * 0.25
            } else {
                0.0
            },
        ),
    );
    center.distance(p) <= reach + pad
}

/// Ставит точку на ближайшую грань стены в пределах `radius`: центр на грани,
/// лицом в помещение. Возвращает центр и поворот.
#[must_use]
pub fn snap_to_face(
    doc: &Document,
    derived: &Derived,
    p: DVec2,
    radius: f64,
) -> Option<(DVec2, f64)> {
    let mut best: Option<(f64, DVec2, f64)> = None;
    for w in doc
        .walls
        .iter()
        .filter(|w| derived.visible.contains(&w.layer))
    {
        let Some((a, b)) = derived.wall_axis(w) else {
            continue;
        };
        let len = a.distance(b);
        if len < 1.0 {
            continue;
        }
        let dir = (b - a) / len;
        let n = left_normal(dir);
        let (left, right) = w.sides();
        for (origin, out) in [(a + n * left, n), (a - n * right, -n)] {
            let along = (p - origin).dot(dir).clamp(0.0, len);
            let off = (p - origin).dot(out);
            if off >= -1.0 && off <= radius && best.is_none_or(|(e, ..)| off < e) {
                let at = origin + dir * along + out * (SYMBOL / 2.0);
                best = Some((off, at, crate::furniture::rotation_facing(out)));
            }
        }
    }
    best.map(|(_, at, rotation)| (at, rotation))
}

/// Каталог для UI.
#[must_use]
pub fn catalog_entries() -> Vec<DeviceEntry> {
    DEVICES
        .iter()
        .map(|i| {
            let k = 90.0
                / (SYMBOL
                    * if i.kind == DeviceKind::LedStrip {
                        5.0
                    } else {
                        1.4
                    });
            let icon = symbol(i.kind)
                .iter()
                .map(|m| {
                    let mut s = String::new();
                    for (j, p) in m.points.iter().enumerate() {
                        let q = DVec2::splat(50.0) + *p * k;
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
            DeviceEntry {
                kind: i.kind,
                name: i.name.into(),
                group: i.group.into(),
                lighting: i.kind.is_light() || is_switch(i.kind),
                on_wall: on_wall(i.kind),
                height: i.height,
                power: i.power,
                flux: i.flux,
                icon,
            }
        })
        .collect()
}

/// Помещение, в котором стоит точка: название и область.
fn room_at<'a>(doc: &'a Document, derived: &Derived, p: DVec2) -> Option<&'a op_model::Room> {
    let region = op_geom::shapes::region_at(&derived.regions, p)?;
    doc.rooms
        .iter()
        .find(|r| derived.room_regions.get(&r.id) == Some(&region))
}

/// Нагрузка на группу.
fn loads(kind: DeviceKind) -> bool {
    !matches!(
        kind,
        DeviceKind::SocketData
            | DeviceKind::SocketTv
            | DeviceKind::JunctionBox
            | DeviceKind::Panel
            | DeviceKind::CeilingOutlet
    ) && !is_switch(kind)
}

fn new_circuit(
    id: String,
    n: usize,
    purpose: String,
    kind: CircuitKind,
    power: f64,
    phases: u8,
) -> Circuit {
    let cos = cos_phi(kind);
    let demand = demand_factor(kind);
    let (breaker, section, rcd) = match kind {
        CircuitKind::Lighting => (10.0, 1.5, false),
        CircuitKind::Sockets => (16.0, 2.5, true),
        CircuitKind::Power => {
            let b =
                electrical::breaker_for(electrical::current(power * demand, phases, cos) * 1.15);
            (b, electrical::section_for(b), phases == 1)
        }
    };
    Circuit {
        id,
        name: format!("Гр.{n}"),
        purpose,
        kind,
        phases,
        breaker,
        rcd,
        section,
        demand,
    }
}

fn cos_phi(kind: CircuitKind) -> f64 {
    match kind {
        CircuitKind::Lighting => 0.95,
        CircuitKind::Sockets => 0.9,
        CircuitKind::Power => 1.0,
    }
}

fn demand_factor(kind: CircuitKind) -> f64 {
    match kind {
        CircuitKind::Lighting | CircuitKind::Power => 1.0,
        CircuitKind::Sockets => 0.8,
    }
}

/// Группа с назначением `purpose` или новая, созданная `make`.
fn find_or_add(
    circuits: &mut Vec<Circuit>,
    purpose: &str,
    new_id: &mut impl FnMut() -> String,
    make: impl FnOnce(String, usize) -> Circuit,
) -> String {
    if let Some(c) = circuits.iter().find(|c| c.purpose == purpose) {
        return c.id.clone();
    }
    let c = make(new_id(), circuits.len() + 1);
    let id = c.id.clone();
    circuits.push(c);
    id
}

/// Группы по умолчанию: освещение, розетки по помещениям, влажные зоны отдельно,
/// каждый силовой потребитель — своя группа. Возвращает группы и точки с группами.
#[must_use]
pub fn auto_circuits(
    doc: &Document,
    derived: &Derived,
    mut new_id: impl FnMut() -> String,
) -> (Vec<Circuit>, Vec<Device>) {
    let mut circuits: Vec<Circuit> = Vec::new();
    let mut devices = Vec::new();
    let shown = |d: &&Device| derived.visible.contains(&d.layer) && loads(d.kind);
    // Освещение — первой группой.
    for dev in doc
        .devices
        .iter()
        .filter(shown)
        .filter(|d| d.kind.is_light())
    {
        let id = find_or_add(&mut circuits, "Освещение", &mut new_id, |id, n| {
            new_circuit(id, n, "Освещение".into(), CircuitKind::Lighting, 0.0, 1)
        });
        devices.push(Device {
            circuit: Some(id),
            ..dev.clone()
        });
    }
    for dev in doc
        .devices
        .iter()
        .filter(shown)
        .filter(|d| !d.kind.is_light())
    {
        let id = match dev.kind {
            DeviceKind::SocketPower | DeviceKind::CableOutlet => {
                let label = if dev.label.trim().is_empty() {
                    kind_name(dev.kind).to_owned()
                } else {
                    dev.label.clone()
                };
                let phases = if dev.kind == DeviceKind::SocketPower {
                    3
                } else {
                    1
                };
                let c = new_circuit(
                    new_id(),
                    circuits.len() + 1,
                    label,
                    CircuitKind::Power,
                    dev.power,
                    phases,
                );
                let id = c.id.clone();
                circuits.push(c);
                id
            }
            DeviceKind::SocketWaterproof => {
                let purpose = "Розетки влажных зон";
                find_or_add(&mut circuits, purpose, &mut new_id, |id, n| {
                    new_circuit(id, n, purpose.into(), CircuitKind::Sockets, 0.0, 1)
                })
            }
            _ => {
                let room = room_at(doc, derived, DVec2::new(dev.x, dev.y)).map(|r| r.name.clone());
                let purpose =
                    room.map_or_else(|| "Розетки".to_owned(), |r| format!("Розетки: {r}"));
                find_or_add(&mut circuits, &purpose.clone(), &mut new_id, |id, n| {
                    new_circuit(id, n, purpose, CircuitKind::Sockets, 0.0, 1)
                })
            }
        };
        devices.push(Device {
            circuit: Some(id),
            ..dev.clone()
        });
    }
    (circuits, devices)
}

/// Трасса группы: от щита по точкам в порядке ближайшего соседа, участки — по
/// горизонтали и вертикали. Возвращает ломаную и длину кабеля с запасом, м.
#[must_use]
pub fn route(doc: &Document, circuit: &str) -> (Vec<DVec2>, f64) {
    let ceiling = doc.default_ceiling_height;
    let mut rest: Vec<&Device> = doc
        .devices
        .iter()
        .filter(|d| d.circuit.as_deref() == Some(circuit))
        .collect();
    if rest.is_empty() {
        return (Vec::new(), 0.0);
    }
    let panel = doc.devices.iter().find(|d| d.kind == DeviceKind::Panel);
    let start = panel.map_or_else(
        || DVec2::new(rest[0].x, rest[0].y),
        |p| DVec2::new(p.x, p.y),
    );
    let drop = |d: &Device| (ceiling - CEILING_RUN - d.height).max(0.0);
    let mut length = panel.map_or(0.0, drop);
    let mut path = vec![start];
    let mut at = start;
    while !rest.is_empty() {
        let (i, _) = rest
            .iter()
            .enumerate()
            .map(|(i, d)| (i, (DVec2::new(d.x, d.y) - at).abs().element_sum()))
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .unwrap_or((0, 0.0));
        let d = rest.remove(i);
        let to = DVec2::new(d.x, d.y);
        path.push(DVec2::new(to.x, at.y));
        path.push(to);
        length += (to - at).abs().element_sum();
        // К настенной точке кабель спускается от трассы под потолком; от промежуточных — и поднимается обратно.
        if on_wall(d.kind) {
            length += drop(d) * if rest.is_empty() { 1.0 } else { 2.0 };
        }
        at = to;
    }
    (path, length * CABLE_RESERVE / 1000.0)
}

/// Предупреждения по нарушениям.
fn warning(issue: electrical::Issue, calc: &electrical::CircuitCalc, c: &Circuit) -> String {
    match issue {
        electrical::Issue::Overload => format!(
            "расчётный ток {:.1} А больше тока автомата {:.0} А",
            calc.current, c.breaker
        ),
        electrical::Issue::CableUnprotected => format!(
            "автомат {:.0} А не защищает кабель {} мм² (допустимо {:.0} А)",
            c.breaker,
            c.section.to_string().replace('.', ","),
            calc.ampacity
        ),
        electrical::Issue::VoltageDrop => format!(
            "падение напряжения {:.1} % больше {:.0} %",
            calc.drop,
            electrical::MAX_VOLTAGE_DROP
        ),
    }
}

/// Расчёт электрики вида: группы, щит, освещённость помещений.
#[must_use]
pub fn report(doc: &Document, derived: &Derived) -> ElectricalReport {
    let known: HashSet<&str> = doc.circuits.iter().map(|c| c.id.as_str()).collect();
    let unassigned = doc
        .devices
        .iter()
        .filter(|d| loads(d.kind) && d.circuit.as_deref().is_none_or(|c| !known.contains(c)))
        .count();
    let mut total_current = 0.0;
    let circuits: Vec<CircuitReport> = doc
        .circuits
        .iter()
        .map(|c| {
            let members: Vec<&Device> = doc
                .devices
                .iter()
                .filter(|d| d.circuit.as_deref() == Some(&c.id))
                .collect();
            let installed: f64 = members.iter().map(|d| d.power).sum();
            let (_, length) = route(doc, &c.id);
            let calc = electrical::circuit(&electrical::CircuitInput {
                installed,
                demand: c.demand,
                cos_phi: cos_phi(c.kind),
                phases: c.phases,
                length,
                section: c.section,
                breaker: c.breaker,
            });
            total_current += calc.current;
            CircuitReport {
                id: c.id.clone(),
                name: c.name.clone(),
                purpose: c.purpose.clone(),
                devices: u32::try_from(members.len()).unwrap_or(u32::MAX),
                installed,
                demand: calc.demand,
                current: calc.current,
                length,
                drop: calc.drop,
                ampacity: calc.ampacity,
                warnings: calc.issues.iter().map(|i| warning(*i, &calc, c)).collect(),
            }
        })
        .collect();
    let lines: Vec<electrical::PanelLine> = doc
        .circuits
        .iter()
        .map(|c| electrical::PanelLine {
            phases: c.phases,
            breaker: c.breaker,
            rcd: c.rcd,
        })
        .collect();
    // Вводной ток — с общим коэффициентом одновременности 0,7 для квартиры.
    let panel = electrical::panel(&lines, total_current * 0.7);
    let mut used = HashSet::new();
    let rooms = doc
        .rooms
        .iter()
        .filter(|r| derived.visible.contains(&r.layer))
        .filter_map(|r| {
            let i = *derived.room_regions.get(&r.id)?;
            used.insert(i).then_some((r, i))
        })
        .map(|(r, i)| {
            let region = &derived.regions[i];
            let area = op_geom::mm2_to_m2(region.area);
            let flux: f64 = doc
                .devices
                .iter()
                .filter(|d| d.kind.is_light() && derived.visible.contains(&d.layer))
                .filter(|d| region.contains(DVec2::new(d.x, d.y)))
                .map(|d| d.flux)
                .sum();
            let ceiling = r.ceiling_height.unwrap_or(doc.default_ceiling_height);
            RoomLighting {
                id: r.id.clone(),
                name: r.name.clone(),
                area,
                flux,
                lux: lighting::illuminance(flux, area, region.perimeter / 1000.0, ceiling),
                norm: lighting::norm(&r.name),
            }
        })
        .collect();
    ElectricalReport {
        circuits,
        panel: PanelReport {
            items: panel
                .items
                .iter()
                .map(|i| PanelItemInfo {
                    name: i.name.clone(),
                    modules: i.modules,
                    count: i.count,
                })
                .collect(),
            modules: panel.modules,
            enclosure: panel.enclosure,
            three_phase: panel.three_phase,
            main_breaker: panel.main_breaker,
        },
        rooms,
        unassigned: u32::try_from(unassigned).unwrap_or(u32::MAX),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_has_symbols_and_unique_kinds() {
        let mut kinds: Vec<DeviceKind> = DEVICES.iter().map(|i| i.kind).collect();
        kinds.dedup();
        assert_eq!(kinds.len(), DEVICES.len());
        for i in DEVICES {
            assert!(!symbol(i.kind).is_empty(), "{:?}", i.kind);
        }
        assert!(catalog_entries().iter().all(|e| e.icon.starts_with('M')));
        assert!(on_wall(DeviceKind::Socket) && !on_wall(DeviceKind::CeilingLight));
        assert!(is_outlet(DeviceKind::FloorOutlet) && is_switch(DeviceKind::PassSwitch));
    }
}
