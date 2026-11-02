//! Отделка: библиотека материалов, поверхности помещений с площадями, раскладка
//! плитки и досок на полу и стенах, подбор старта, расход материалов.
//!
//! Пол и потолок — область помещения (колонны вырезаны), стены — грани контура
//! по развёртке (проёмы вырезаны). Отделки стен одного помещения перекрываются по
//! порядку списка: следующая закрывает предыдущую в своей полосе высот.

use std::borrow::Cow;

use glam::DVec2;
use op_calc::finish as calc;
use op_geom::tiling::{self, Pattern, Spec, Surface as Area};
use op_model::{Document, Finish, Material, MaterialKind, Surface, TilePattern};
use op_render::scene::Color;

use crate::{
    derived::Derived,
    protocol::{FaceInfo, FinishInfo, FinishReport, MaterialTotal, Quantity, RoomSurfaces},
    surfaces::{self, Face},
};

const fn m(
    id: &'static str,
    name: &'static str,
    kind: MaterialKind,
    size: (f64, f64, f64),
    pack: f64,
    color: &'static str,
) -> (
    &'static str,
    &'static str,
    MaterialKind,
    (f64, f64, f64),
    f64,
    &'static str,
) {
    (id, name, kind, size, pack, color)
}

type Entry = (
    &'static str,
    &'static str,
    MaterialKind,
    (f64, f64, f64),
    f64,
    &'static str,
);

/// Встроенная библиотека: идентификатор, название, вид, ширина × длина × толщина,
/// в упаковке, цвет.
const LIBRARY: &[Entry] = &[
    m(
        "tile-300x600",
        "Плитка настенная 300×600",
        MaterialKind::Tile,
        (300.0, 600.0, 8.5),
        8.0,
        "#F1F0EB",
    ),
    m(
        "tile-200x200",
        "Плитка 200×200",
        MaterialKind::Tile,
        (200.0, 200.0, 8.0),
        25.0,
        "#DCE3E8",
    ),
    m(
        "porcelain-600x600",
        "Керамогранит 600×600",
        MaterialKind::Tile,
        (600.0, 600.0, 9.0),
        4.0,
        "#B9B4AA",
    ),
    m(
        "porcelain-600x1200",
        "Керамогранит 600×1200",
        MaterialKind::Tile,
        (600.0, 1200.0, 9.0),
        2.0,
        "#8F8A82",
    ),
    m(
        "porcelain-wood",
        "Керамогранит под дерево 200×1200",
        MaterialKind::Tile,
        (200.0, 1200.0, 9.0),
        6.0,
        "#A57C55",
    ),
    m(
        "mosaic-300x300",
        "Мозаика, лист 300×300",
        MaterialKind::Tile,
        (300.0, 300.0, 4.0),
        10.0,
        "#6FA3B8",
    ),
    m(
        "laminate-oak",
        "Ламинат 33 класс, дуб",
        MaterialKind::Laminate,
        (193.0, 1380.0, 8.0),
        2.131,
        "#C8A273",
    ),
    m(
        "parquet-board",
        "Паркетная доска",
        MaterialKind::Laminate,
        (188.0, 2266.0, 14.0),
        2.56,
        "#9C6B43",
    ),
    m(
        "paint-white",
        "Краска интерьерная белая, 9 л",
        MaterialKind::Paint,
        (0.0, 0.0, 0.0),
        9.0,
        "#F7F6F1",
    ),
    m(
        "paint-color",
        "Краска интерьерная колерованная, 2,7 л",
        MaterialKind::Paint,
        (0.0, 0.0, 0.0),
        2.7,
        "#D9CBB5",
    ),
    m(
        "wallpaper",
        "Обои флизелиновые 1,06 × 10,05 м",
        MaterialKind::Wallpaper,
        (1060.0, 10050.0, 0.0),
        1.0,
        "#E9E2D0",
    ),
    m(
        "stretch-matte",
        "Натяжной потолок матовый",
        MaterialKind::Sheet,
        (0.0, 0.0, 0.0),
        0.0,
        "#FFFFFF",
    ),
    m(
        "carpet",
        "Ковролин",
        MaterialKind::Sheet,
        (0.0, 0.0, 0.0),
        0.0,
        "#8E8B84",
    ),
];

/// Встроенная библиотека материалов.
#[must_use]
pub fn library() -> Vec<Material> {
    LIBRARY
        .iter()
        .map(|(id, name, kind, (w, l, t), pack, color)| Material {
            id: (*id).into(),
            name: (*name).into(),
            kind: *kind,
            width: *w,
            length: *l,
            thickness: *t,
            pack: *pack,
            color: (*color).into(),
        })
        .collect()
}

/// Материал проекта или библиотеки.
#[must_use]
pub fn material<'a>(doc: &'a Document, id: &str) -> Option<Cow<'a, Material>> {
    doc.materials
        .iter()
        .find(|m| m.id == id)
        .map(Cow::Borrowed)
        .or_else(|| library().into_iter().find(|m| m.id == id).map(Cow::Owned))
}

/// Цвет материала «#rrggbb».
#[must_use]
pub fn color(material: &Material) -> Color {
    let hex = material.color.trim_start_matches('#');
    u32::from_str_radix(hex, 16)
        .ok()
        .filter(|_| hex.len() == 6)
        .map_or(Color::hex(0xDDDDDD, 1.0), |v| Color::hex(v, 1.0))
}

/// Название поверхности для ведомостей.
#[must_use]
pub(crate) fn surface_name(f: &Finish, faces: &[Face]) -> String {
    match (f.surface, f.face) {
        (Surface::Floor, _) => "Пол".into(),
        (Surface::Ceiling, _) => "Потолок".into(),
        (Surface::Walls, None) => "Стены".into(),
        (Surface::Walls, Some(k)) => format!(
            "Стена {}",
            usize::try_from(k)
                .ok()
                .and_then(|k| faces.get(k))
                .map_or("?", |f| f.letter.as_str())
        ),
    }
}

/// Помещение для отделки: область и грани.
pub(crate) struct RoomGeometry {
    /// Контур.
    pub contour: Vec<DVec2>,
    /// Колонны и другие вырезы пола.
    pub islands: Vec<Vec<DVec2>>,
    /// Грани по развёртке.
    pub faces: Vec<Face>,
    /// Высота помещения, мм.
    pub height: f64,
    /// Площадь пола, мм².
    pub area: f64,
}

pub(crate) fn room_geometry(doc: &Document, derived: &Derived, room: &str) -> Option<RoomGeometry> {
    let r = doc.room(room)?;
    let region = &derived.regions[*derived.room_regions.get(room)?];
    let (contour, faces) = surfaces::room_faces(doc, derived, room)?;
    Some(RoomGeometry {
        contour,
        islands: region.islands.clone(),
        faces,
        height: r.ceiling_height.unwrap_or(doc.default_ceiling_height),
        area: region.area,
    })
}

/// Полоса стены, занятая отделкой: грань и высоты.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Band {
    /// Номер грани.
    pub face: usize,
    /// Низ, мм.
    pub bottom: f64,
    /// Верх, мм.
    pub top: f64,
}

fn subtract(intervals: Vec<(f64, f64)>, cut: (f64, f64)) -> Vec<(f64, f64)> {
    let mut out = Vec::new();
    for (a, b) in intervals {
        if cut.1 <= a || cut.0 >= b {
            out.push((a, b));
            continue;
        }
        if cut.0 > a {
            out.push((a, cut.0));
        }
        if cut.1 < b {
            out.push((cut.1, b));
        }
    }
    out
}

/// Полосы стен, которые отделка действительно закрывает (с учётом следующих отделок).
pub(crate) fn wall_bands(doc: &Document, finish: &Finish, room: &RoomGeometry) -> Vec<Band> {
    let applies = |f: &Finish, k: usize| {
        f.room == finish.room
            && f.surface == Surface::Walls
            && f.face.is_none_or(|n| usize::try_from(n).ok() == Some(k))
    };
    let band = |f: &Finish| (f.bottom, f.top.unwrap_or(room.height).min(room.height));
    let Some(index) = doc.finishes.iter().position(|f| f.id == finish.id) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for k in 0..room.faces.len() {
        if !applies(finish, k) {
            continue;
        }
        let mut free = vec![band(finish)];
        for later in doc.finishes[index + 1..].iter().filter(|f| applies(f, k)) {
            let cut = band(later);
            free = free.into_iter().fold(Vec::new(), |mut acc, i| {
                acc.extend(subtract(vec![i], cut));
                acc
            });
        }
        out.extend(
            free.into_iter()
                .filter(|(a, b)| b - a > 1.0)
                .map(|(bottom, top)| Band {
                    face: k,
                    bottom,
                    top,
                }),
        );
    }
    out
}

/// Поверхность полосы стены в координатах грани (u — вдоль, v — вверх от пола).
pub(crate) fn band_area(face: &Face, band: &Band) -> Area {
    let mut area = Area::rect(
        DVec2::new(0.0, band.bottom),
        DVec2::new(face.length, band.top),
    );
    for o in &face.openings {
        let (lo, hi) = (o.bottom.max(band.bottom), o.top.min(band.top));
        if hi - lo > 1.0 {
            area.holes.push(vec![
                DVec2::new(o.from, lo),
                DVec2::new(o.to, lo),
                DVec2::new(o.to, hi),
                DVec2::new(o.from, hi),
            ]);
        }
    }
    area
}

/// Последняя по списку отделка пола (потолка) помещения — она и действует.
fn effective(doc: &Document, finish: &Finish) -> bool {
    finish.surface == Surface::Walls
        || doc
            .finishes
            .iter()
            .rev()
            .find(|f| f.room == finish.room && f.surface == finish.surface)
            .is_some_and(|f| f.id == finish.id)
}

/// Поверхности отделки и старт раскладки в их координатах.
pub(crate) fn areas(doc: &Document, finish: &Finish, room: &RoomGeometry) -> (Vec<Area>, DVec2) {
    if !effective(doc, finish) {
        return (Vec::new(), DVec2::ZERO);
    }
    match finish.surface {
        Surface::Floor | Surface::Ceiling => {
            let lo = room
                .contour
                .iter()
                .fold(DVec2::splat(f64::INFINITY), |a, p| a.min(*p));
            (
                vec![Area {
                    outer: room.contour.clone(),
                    holes: room.islands.clone(),
                }],
                lo + DVec2::from(finish.layout.origin),
            )
        }
        Surface::Walls => (
            wall_bands(doc, finish, room)
                .iter()
                .map(|b| band_area(&room.faces[b.face], b))
                .collect(),
            DVec2::from(finish.layout.origin),
        ),
    }
}

/// Раскладка штучного материала: длина элемента — вдоль ряда.
pub(crate) fn spec(material: &Material, finish: &Finish, origin: DVec2) -> Option<Spec> {
    let piecewise = matches!(material.kind, MaterialKind::Tile | MaterialKind::Laminate);
    (piecewise && material.width > 0.0 && material.length > 0.0).then(|| Spec {
        length: material.length,
        width: material.width,
        joint: if material.kind == MaterialKind::Tile {
            finish.layout.joint
        } else {
            0.0
        },
        pattern: match finish.layout.pattern {
            TilePattern::Straight => Pattern::Straight,
            TilePattern::Offset => Pattern::Offset(finish.layout.shift),
            TilePattern::Diagonal => Pattern::Diagonal,
        },
        origin,
    })
}

/// Количество с размером упаковки — чтобы итоги считали упаковки по сумме.
#[derive(Debug, Clone)]
struct Amount {
    name: &'static str,
    amount: f64,
    unit: &'static str,
    per_pack: f64,
}

impl Amount {
    fn quantity(&self) -> Quantity {
        Quantity {
            name: self.name.into(),
            amount: (self.amount * 100.0).round() / 100.0,
            unit: self.unit.into(),
            packs: calc::packs(self.amount, self.per_pack),
        }
    }
}

/// Расчёт одной отделки.
struct Computed {
    info: FinishInfo,
    amounts: Vec<Amount>,
    area: f64,
}

fn compute(doc: &Document, derived: &Derived, finish: &Finish) -> Computed {
    let mut warnings = Vec::new();
    let empty = |warnings: Vec<String>| Computed {
        info: FinishInfo {
            id: finish.id.clone(),
            area: 0.0,
            whole: 0,
            cut: 0,
            narrow: 0,
            quantities: Vec::new(),
            warnings,
        },
        amounts: Vec::new(),
        area: 0.0,
    };
    let Some(room) = room_geometry(doc, derived, &finish.room) else {
        return empty(vec!["помещение не найдено или контур не замкнут".into()]);
    };
    let Some(material) = material(doc, &finish.material) else {
        return empty(vec!["материал не найден".into()]);
    };
    if !effective(doc, finish) {
        return empty(vec!["перекрыта отделкой ниже по списку".into()]);
    }
    let (surfaces, origin) = areas(doc, finish, &room);
    let area_mm: f64 = surfaces.iter().map(Area::area).sum();
    let area = op_geom::mm2_to_m2(area_mm);
    let mut info = FinishInfo {
        id: finish.id.clone(),
        area: (area * 100.0).round() / 100.0,
        whole: 0,
        cut: 0,
        narrow: 0,
        quantities: Vec::new(),
        warnings: Vec::new(),
    };
    let mut amounts = Vec::new();
    match material.kind {
        MaterialKind::Tile => {
            if let Some(spec) = spec(&material, finish, origin) {
                let layout = tiling::layout_all(&surfaces, &spec, false);
                info.whole = u32::try_from(layout.whole).unwrap_or(u32::MAX);
                info.cut = u32::try_from(layout.cut).unwrap_or(u32::MAX);
                info.narrow = u32::try_from(layout.narrow).unwrap_or(u32::MAX);
                if layout.narrow > 0 {
                    warnings.push(format!(
                        "узкие подрезки (уже ¼ плитки): {} шт. — подберите старт",
                        layout.narrow
                    ));
                }
                if layout.approximate {
                    warnings.push("мелкий элемент: количество посчитано по площади".into());
                }
                amounts.push(Amount {
                    name: "Плитка",
                    amount: f64::from(calc::tiles_with_reserve(layout.tiles())),
                    unit: "шт.",
                    per_pack: material.pack,
                });
                amounts.push(Amount {
                    name: "Клей",
                    amount: area * calc::adhesive_rate(material.width, material.length),
                    unit: "кг",
                    per_pack: calc::ADHESIVE_BAG,
                });
                amounts.push(Amount {
                    name: "Затирка",
                    amount: area
                        * calc::grout_rate(
                            material.width,
                            material.length,
                            material.thickness.max(1.0),
                            finish.layout.joint,
                        ),
                    unit: "кг",
                    per_pack: calc::GROUT_PACK,
                });
            }
        }
        MaterialKind::Laminate => {
            if let Some(spec) = spec(&material, finish, origin) {
                let layout = tiling::layout_all(&surfaces, &spec, false);
                info.whole = u32::try_from(layout.whole).unwrap_or(u32::MAX);
                info.cut = u32::try_from(layout.cut).unwrap_or(u32::MAX);
                info.narrow = u32::try_from(layout.narrow).unwrap_or(u32::MAX);
                if layout.narrow > 0 {
                    warnings.push(format!(
                        "узкие подрезки досок: {} шт. — подберите старт",
                        layout.narrow
                    ));
                }
            }
            amounts.push(Amount {
                name: "Покрытие",
                amount: calc::boards_area(area),
                unit: "м²",
                per_pack: material.pack,
            });
        }
        MaterialKind::Paint => amounts.push(Amount {
            name: "Краска",
            amount: calc::paint_liters(area, calc::PAINT_COATS),
            unit: "л",
            per_pack: material.pack,
        }),
        MaterialKind::Wallpaper => {
            let bands = wall_bands(doc, finish, &room);
            let perimeter: f64 = room
                .faces
                .iter()
                .enumerate()
                .filter(|(k, _)| bands.iter().any(|b| b.face == *k))
                .map(|(_, f)| f.length)
                .sum();
            let height = bands.iter().map(|b| b.top - b.bottom).fold(0.0, f64::max);
            let rolls = if finish.surface == Surface::Walls {
                calc::wallpaper_rolls(perimeter, height, material.width, material.length)
            } else {
                calc::packs(area_mm, material.width * material.length)
            };
            amounts.push(Amount {
                name: "Обои",
                amount: f64::from(rolls),
                unit: "рул.",
                per_pack: 0.0,
            });
        }
        MaterialKind::Sheet => amounts.push(Amount {
            name: "Покрытие",
            amount: area,
            unit: "м²",
            per_pack: material.pack,
        }),
    }
    info.quantities = amounts.iter().map(Amount::quantity).collect();
    info.warnings = warnings;
    Computed {
        info,
        amounts,
        area,
    }
}

/// Поверхности помещений вида.
fn rooms(doc: &Document, derived: &Derived) -> Vec<RoomSurfaces> {
    let mut used = std::collections::HashSet::new();
    doc.rooms
        .iter()
        .filter(|r| derived.visible.contains(&r.layer))
        .filter(|r| {
            derived
                .room_regions
                .get(&r.id)
                .is_some_and(|i| used.insert(*i))
        })
        .filter_map(|r| {
            let g = room_geometry(doc, derived, &r.id)?;
            let perimeter: f64 = g.faces.iter().map(|f| f.length).sum::<f64>() / 1000.0;
            let height = g.height / 1000.0;
            let openings: f64 = g
                .faces
                .iter()
                .flat_map(|f| &f.openings)
                .map(|o| (o.to - o.from) * (o.top.min(g.height) - o.bottom).max(0.0) / 1e6)
                .sum();
            let floor = op_geom::mm2_to_m2(g.area);
            let round = |v: f64| (v * 100.0).round() / 100.0;
            Some(RoomSurfaces {
                id: r.id.clone(),
                name: r.name.clone(),
                floor: round(floor),
                ceiling: round(floor),
                perimeter: round(perimeter),
                height: round(height),
                walls_gross: round(perimeter * height),
                walls_net: round(perimeter * height - openings),
                faces: g
                    .faces
                    .iter()
                    .map(|f| FaceInfo {
                        letter: f.letter.clone(),
                        length: f.length.round(),
                    })
                    .collect(),
            })
        })
        .collect()
}

/// Расчёт отделки вида: поверхности помещений, отделки, итоги по материалам.
#[must_use]
pub fn report(doc: &Document, derived: &Derived) -> FinishReport {
    let computed: Vec<(&Finish, Computed)> = doc
        .finishes
        .iter()
        .filter(|f| doc.room(&f.room).is_some())
        .map(|f| (f, compute(doc, derived, f)))
        .collect();
    let mut totals: Vec<(String, Material, f64, Vec<Amount>)> = Vec::new();
    for (f, c) in &computed {
        let Some(material) = material(doc, &f.material) else {
            continue;
        };
        let at = match totals.iter().position(|t| t.0 == f.material) {
            Some(i) => i,
            None => {
                totals.push((f.material.clone(), material.into_owned(), 0.0, Vec::new()));
                totals.len() - 1
            }
        };
        let total = &mut totals[at];
        total.2 += c.area;
        for a in &c.amounts {
            match total.3.iter_mut().find(|t| t.name == a.name) {
                Some(t) => t.amount += a.amount,
                None => total.3.push(a.clone()),
            }
        }
    }
    FinishReport {
        rooms: rooms(doc, derived),
        finishes: computed.into_iter().map(|(_, c)| c.info).collect(),
        totals: totals
            .into_iter()
            .map(|(id, m, area, amounts)| MaterialTotal {
                material: id,
                name: m.name,
                kind: m.kind,
                area: (area * 100.0).round() / 100.0,
                quantities: amounts.iter().map(Amount::quantity).collect(),
            })
            .collect(),
    }
}

/// Старт раскладки с наименьшим числом узких подрезок: новая раскладка отделки.
#[must_use]
pub fn optimize(doc: &Document, derived: &Derived, id: &str) -> Option<Finish> {
    let finish = doc.finishes.iter().find(|f| f.id == id)?;
    let room = room_geometry(doc, derived, &finish.room)?;
    let material = material(doc, &finish.material)?;
    let (surfaces, origin) = areas(doc, finish, &room);
    let spec = spec(&material, finish, origin)?;
    let best = tiling::best_origin(&surfaces, &spec);
    let base = match finish.surface {
        Surface::Walls => DVec2::ZERO,
        _ => room
            .contour
            .iter()
            .fold(DVec2::splat(f64::INFINITY), |a, p| a.min(*p)),
    };
    let rel = ((best - base) * 10.0).round() / 10.0;
    let mut out = finish.clone();
    out.layout.origin = [rel.x, rel.y];
    Some(out)
}

/// Раскладка пола на плане: заливки помещений цветом материала и контуры элементов.
#[derive(Debug, Clone, Default)]
pub struct PlanTiles {
    /// Области пола с цветом материала.
    pub fills: Vec<(Vec<Vec<DVec2>>, Color)>,
    /// Контуры элементов.
    pub pieces: Vec<Vec<DVec2>>,
}

/// Раскладка полов вида для плана.
#[must_use]
pub fn plan_tiles(doc: &Document, derived: &Derived) -> PlanTiles {
    let mut out = PlanTiles::default();
    for f in doc
        .finishes
        .iter()
        .filter(|f| f.surface == Surface::Floor && effective(doc, f))
    {
        let (Some(room), Some(material)) = (
            room_geometry(doc, derived, &f.room),
            material(doc, &f.material),
        ) else {
            continue;
        };
        let mut contours = vec![room.contour.clone()];
        contours.extend(room.islands.iter().cloned());
        out.fills.push((contours, color(&material)));
        let (surfaces, origin) = areas(doc, f, &room);
        if let Some(spec) = spec(&material, f, origin) {
            let layout = tiling::layout_all(&surfaces, &spec, true);
            if !layout.approximate {
                out.pieces
                    .extend(layout.pieces.into_iter().map(|p| p.outline));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn library_is_unique_and_resolvable() {
        let lib = library();
        let mut ids: Vec<&str> = lib.iter().map(|m| m.id.as_str()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), lib.len());
        let doc = Document::new("");
        assert!(material(&doc, "tile-300x600").is_some());
        assert!(material(&doc, "нет").is_none());
    }

    #[test]
    fn subtract_cuts_bands() {
        assert_eq!(
            subtract(vec![(0.0, 2700.0)], (0.0, 1500.0)),
            vec![(1500.0, 2700.0)]
        );
        assert_eq!(
            subtract(vec![(0.0, 2700.0)], (900.0, 1500.0)),
            vec![(0.0, 900.0), (1500.0, 2700.0)]
        );
    }
}
