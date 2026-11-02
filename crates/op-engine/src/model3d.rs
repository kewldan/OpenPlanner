//! 3D-модель вида: стены с вырезами проёмов, перемычки и подоконные части,
//! дверные полотна, стёкла, полы и потолки помещений.
//!
//! Стена — призма своего контура со стыками минус проёмы, на свою высоту; над
//! проёмом — перемычка, под окном — подоконная часть. Срез по высоте обрезает
//! всё выше отметки и закрашивает срез тёмным, как на плане.

use std::collections::HashSet;

use glam::DVec2;
use op_geom::shapes;
use op_model::{Document, OpeningKind, Side, WallMaterial};
use op_render::{
    scene::Color,
    three::{Mesh, Scene3D, Sun, from_plan},
};

use crate::{
    derived::Derived,
    draw::Models,
    geometry,
    protocol::Options3d,
    sun::{self, SunPosition},
};

/// Толщина дверного полотна, мм.
const LEAF: f64 = 40.0;
/// Толщина стекла, мм.
const GLASS: f64 = 12.0;

/// Цвета модели: материалы или белая модель.
struct Palette {
    wall: Color,
    floor: Color,
    ceiling: Color,
    door: Color,
    section: Color,
    glass: Color,
    white: bool,
}

impl Palette {
    fn new(white: bool) -> Self {
        let c = |rgb: u32| Color::hex(rgb, 1.0);
        Self {
            wall: c(if white { 0xF2F2F0 } else { 0xEDE9E2 }),
            floor: c(if white { 0xE4E4E1 } else { 0xCDB08A }),
            ceiling: c(0xF7F7F5),
            door: c(if white { 0xE9E9E6 } else { 0xB89A76 }),
            section: c(0x3B3B37),
            glass: Color::hex(0xA9C6DA, 0.35),
            white,
        }
    }

    /// Новые стены варианта — светлый оттенок цвета материала.
    fn wall(&self, material: Option<WallMaterial>, new: bool) -> Color {
        if self.white || !new {
            return self.wall;
        }
        Color::hex(
            match material {
                Some(WallMaterial::AeratedConcrete) => 0xC9D4E0,
                Some(WallMaterial::Gypsum) => 0xEDE1BE,
                Some(WallMaterial::Brick) => 0xD9A08C,
                None => 0xCFE5D8,
            },
            1.0,
        )
    }
}

fn to3(points: &[DVec2]) -> Vec<DVec2> {
    points.iter().map(|p| from_plan(*p)).collect()
}

fn rings(shape: &shapes::Shape) -> Vec<Vec<DVec2>> {
    std::iter::once(to3(&shape.outer))
        .chain(shape.holes.iter().map(|h| to3(h)))
        .collect()
}

/// Предмет мебели: своя модель — треугольниками, позиция каталога — брусками.
fn furniture(
    mesh: &mut Mesh,
    f: &op_model::Furniture,
    models: &Models,
    options: &Options3d,
    palette: &Palette,
) {
    let top = options.cut.unwrap_or(f64::INFINITY);
    if f.elevation >= top {
        return;
    }
    let tint = |rgb: u32| {
        if palette.white {
            Color::hex(0xECECE9, 1.0)
        } else {
            Color::hex(rgb, 1.0)
        }
    };
    let place = |x: f64, y: f64| from_plan(crate::furniture::to_plan(f, DVec2::new(x, y)));
    if let Some(model) = f.model.as_ref().and_then(|m| models.get(m)) {
        let color = tint(0xB9B3AA);
        for t in &model.triangles {
            let corner = |u: glam::DVec3| {
                let p = place((u.x - 0.5) * f.width, (u.y - 0.5) * f.depth);
                p.extend((f.elevation + u.z * f.height).min(top))
            };
            let [a, b, c] = t.map(corner);
            // Переход из осей glTF (X, Y вверх, Z к зрителю) в оси сцены (X, −Z, Y)
            // сохраняет обход; отражение по ширине его меняет.
            if f.mirrored {
                mesh.triangle([a, c, b], color);
            } else {
                mesh.triangle([a, b, c], color);
            }
        }
        return;
    }
    for part in crate::furniture::parts(&f.item, f.width, f.depth, f.height) {
        let z0 = f.elevation + part.lo.z;
        let z1 = (f.elevation + part.hi.z).min(top);
        if z1 <= z0 {
            continue;
        }
        let rect = [
            place(part.lo.x, part.lo.y),
            place(part.hi.x, part.lo.y),
            place(part.hi.x, part.hi.y),
            place(part.lo.x, part.hi.y),
        ];
        mesh.block(rect, z0, z1, tint(part.color));
    }
}

/// Электроточка: накладка на стене, щит, светильник на потолке или подвес.
fn device(mesh: &mut Mesh, d: &op_model::Device, ceiling: f64, options: &Options3d) {
    use op_model::DeviceKind as K;
    let top = options.cut.unwrap_or(f64::INFINITY);
    let face = -crate::electric::SYMBOL / 2.0;
    // Брусок в локальных осях точки: x, y (стена на −Y), высота.
    let mut part = |x: f64, y0: f64, y1: f64, z0: f64, z1: f64, color: u32| {
        let z1 = z1.min(top);
        if z1 <= z0 {
            return;
        }
        let place = |px: f64, py: f64| from_plan(crate::electric::to_plan(d, DVec2::new(px, py)));
        let rect = [place(-x, y0), place(x, y0), place(x, y1), place(-x, y1)];
        mesh.block(rect, z0, z1, Color::hex(color, 1.0));
    };
    let plate = 0xF7F7F5;
    let glow = 0xFFF1C2;
    match d.kind {
        K::CeilingLight => part(200.0, -200.0, 200.0, ceiling - 60.0, ceiling, glow),
        K::Pendant => {
            part(150.0, -150.0, 150.0, ceiling - 750.0, ceiling - 600.0, glow);
            part(6.0, -6.0, 6.0, ceiling - 600.0, ceiling, 0x555555);
        }
        K::Spot => part(45.0, -45.0, 45.0, ceiling - 10.0, ceiling, glow),
        K::LedStrip => part(500.0, -10.0, 10.0, ceiling - 10.0, ceiling, glow),
        K::CeilingOutlet => part(60.0, -60.0, 60.0, ceiling - 20.0, ceiling, plate),
        K::FloorOutlet => part(60.0, -60.0, 60.0, 0.0, 10.0, plate),
        K::WallLight => part(
            100.0,
            face,
            face + 120.0,
            d.height - 75.0,
            d.height + 75.0,
            glow,
        ),
        K::Panel => part(
            250.0,
            face,
            face + 120.0,
            d.height - 250.0,
            d.height + 250.0,
            0xE3E3E0,
        ),
        K::AcIndoor => part(
            450.0,
            face,
            face + 250.0,
            d.height - 150.0,
            d.height + 150.0,
            0xF4F4F2,
        ),
        K::AcOutdoor => part(
            400.0,
            face,
            face + 300.0,
            d.height - 275.0,
            d.height + 275.0,
            0xD5D5D1,
        ),
        K::Breather => part(
            220.0,
            face,
            face + 150.0,
            d.height - 220.0,
            d.height + 220.0,
            0xF4F4F2,
        ),
        K::VentGrille => part(
            100.0,
            face,
            face + 10.0,
            d.height - 100.0,
            d.height + 100.0,
            0xE2E2DE,
        ),
        K::ExhaustFan => part(
            80.0,
            face,
            face + 40.0,
            d.height - 80.0,
            d.height + 80.0,
            0xF4F4F2,
        ),
        K::JunctionBox => part(
            40.0,
            face,
            face + 30.0,
            d.height - 40.0,
            d.height + 40.0,
            plate,
        ),
        _ => part(
            40.0,
            face,
            face + 15.0,
            d.height - 40.0,
            d.height + 40.0,
            plate,
        ),
    }
}

/// Труба: брусок сечением в диаметр вдоль каждого участка.
fn pipe(mesh: &mut Mesh, pipe: &op_model::Pipe, options: &Options3d, white: bool) {
    let top = options.cut.unwrap_or(f64::INFINITY);
    let color = if white {
        Color::hex(0xDADAD6, 1.0)
    } else {
        match pipe.system {
            op_model::PipeSystem::Cold => Color::hex(0x5B8DEF, 1.0),
            op_model::PipeSystem::Hot => Color::hex(0xE05A5A, 1.0),
            op_model::PipeSystem::Drain => Color::hex(0x9A9A96, 1.0),
        }
    };
    let r = pipe.diameter / 2.0;
    for w in pipe.points.windows(2) {
        let (a, b) = (crate::plumbing::xy(&w[0]), crate::plumbing::xy(&w[1]));
        let z0 = w[0].z.min(w[1].z) - r;
        let z1 = (w[0].z.max(w[1].z) + r).min(top);
        if z1 <= z0 {
            continue;
        }
        let dir = (b - a).try_normalize().unwrap_or(DVec2::X);
        let n = DVec2::new(-dir.y, dir.x) * r;
        let (a, b) = (a - dir * r, b + dir * r);
        let rect = [a - n, b - n, b + n, a + n].map(from_plan);
        mesh.block(rect, z0, z1, color);
    }
}

/// Сантехническая точка: заглушка вывода, стояк до потолка, коллектор.
fn plumbing_point(mesh: &mut Mesh, p: &op_model::PlumbingPoint, ceiling: f64, options: &Options3d) {
    let top = options.cut.unwrap_or(f64::INFINITY);
    let r = p.diameter / 2.0 + 10.0;
    let (z0, z1, w, color) = match p.kind {
        op_model::PlumbingKind::Outlet => (p.height - r, p.height + r, r, 0xF2F2EE),
        op_model::PlumbingKind::Riser => (0.0, ceiling, r, 0xB5B5B0),
        op_model::PlumbingKind::Manifold => (p.height - 40.0, p.height + 40.0, 200.0, 0xC9A227),
    };
    let z1 = z1.min(top);
    if z1 <= z0 {
        return;
    }
    let local = |x: f64, y: f64| from_plan(crate::plumbing::to_plan(p, DVec2::new(x, y)));
    let (y0, y1) = match p.kind {
        op_model::PlumbingKind::Outlet => (0.0, 2.0 * r),
        _ => (-r.min(w), r.min(w)),
    };
    let rect = [local(-w, y0), local(w, y0), local(w, y1), local(-w, y1)];
    mesh.block(rect, z0, z1, Color::hex(color, 1.0));
}

/// Солнце для настроек: положение (в том числе под горизонтом) и свет для сцены.
#[must_use]
pub fn sun(doc: &Document, options: &Options3d) -> Option<(SunPosition, Option<Sun>)> {
    let time = options.sun?;
    let position = sun::position(&doc.site, time.day, time.hour);
    let light = (position.altitude > 0.0).then(|| Sun {
        direction: sun::direction(&doc.site, position),
        // У горизонта свет слабеет.
        intensity: (position.altitude.to_radians().sin() * 4.0).clamp(0.0, 1.0),
    });
    Some((position, light))
}

/// 3D-сцена вида `doc` (обмер или проекция варианта) с его производными данными;
/// `models` — свои модели мебели по путям файлов.
#[must_use]
pub fn build(doc: &Document, derived: &Derived, options: &Options3d, models: &Models) -> Scene3D {
    let palette = Palette::new(options.white);
    let mut opaque = Mesh::default();
    let mut glass = Mesh::default();
    let shown = |layer: &str| derived.visible.contains(layer);
    let default_height = doc.default_ceiling_height;
    let clip = |z: f64| options.cut.map_or(z, |c| z.min(c.max(1.0)));
    let top_color = |c: Color| {
        if options.cut.is_some() {
            palette.section
        } else {
            c
        }
    };

    for w in doc.walls.iter().filter(|w| shown(&w.layer)) {
        let Some(outline) = derived.wall_outlines.get(&w.id).filter(|o| !o.is_empty()) else {
            continue;
        };
        let height = w.height.unwrap_or(default_height);
        let top = clip(height);
        let color = palette.wall(w.material, w.variant.is_some());
        let openings: Vec<_> = doc
            .openings
            .iter()
            .filter(|o| o.wall == w.id && shown(&o.layer))
            .filter_map(|o| Some((o, geometry::opening(doc, derived, o)?)))
            .collect();
        let cuts: Vec<Vec<DVec2>> = openings.iter().map(|(_, g)| g.rect.clone()).collect();
        for shape in shapes::difference(std::slice::from_ref(outline), &cuts) {
            opaque.prism(
                &rings(&shape),
                0.0,
                top,
                color,
                Some(top_color(color)),
                false,
            );
        }
        for (o, g) in &openings {
            let rect = to3(&g.rect);
            let bottom = if o.kind == OpeningKind::Window {
                o.sill
            } else {
                0.0
            };
            let head = (bottom + o.height).min(height);
            // Перемычка над проёмом и подоконная часть под окном.
            if head < top {
                opaque.prism(
                    std::slice::from_ref(&rect),
                    head,
                    top,
                    color,
                    Some(top_color(color)),
                    true,
                );
            }
            if bottom > 0.0 {
                let sill = clip(bottom);
                opaque.prism(
                    std::slice::from_ref(&rect),
                    0.0,
                    sill,
                    color,
                    Some(top_color(color)),
                    false,
                );
            }
            match o.kind {
                OpeningKind::Door => {
                    // Полотно открыто на 90°, как на плане.
                    let (n, l, r) = (g.normal, g.left, g.right);
                    let side = if o.swing == Side::Left { 1.0 } else { -1.0 };
                    let face = if o.swing == Side::Left { l } else { r };
                    let (hinge, free) = if o.hinge_at_end {
                        (g.end, g.start)
                    } else {
                        (g.start, g.end)
                    };
                    let along = (free - hinge).normalize_or_zero();
                    let hinge = hinge + n * side * face;
                    let open = hinge + n * side * hinge.distance(free + n * side * face);
                    let leaf = [hinge, open, open + along * LEAF, hinge + along * LEAF];
                    let leaf_top = clip(o.height.min(height));
                    opaque.prism(
                        &[to3(&leaf)],
                        0.0,
                        leaf_top,
                        palette.door,
                        Some(palette.door),
                        false,
                    );
                }
                OpeningKind::Window if bottom < top => {
                    let mid = g.normal * (g.left - g.right) / 2.0;
                    let half = g.normal * GLASS / 2.0;
                    let pane = [
                        g.start + mid - half,
                        g.end + mid - half,
                        g.end + mid + half,
                        g.start + mid + half,
                    ];
                    glass.prism(
                        &[to3(&pane)],
                        bottom,
                        clip(head),
                        palette.glass,
                        Some(palette.glass),
                        true,
                    );
                }
                OpeningKind::Window | OpeningKind::Arch => {}
            }
        }
    }

    for f in doc.furniture.iter().filter(|f| shown(&f.layer)) {
        furniture(&mut opaque, f, models, options, &palette);
    }
    for d in doc.devices.iter().filter(|d| shown(&d.layer)) {
        device(&mut opaque, d, doc.default_ceiling_height, options);
    }
    for p in doc.pipes.iter().filter(|p| shown(&p.layer)) {
        pipe(&mut opaque, p, options, palette.white);
    }
    for p in doc.plumbing.iter().filter(|p| shown(&p.layer)) {
        plumbing_point(&mut opaque, p, doc.default_ceiling_height, options);
    }

    // Полы и потолки помещений (метки в одной области — одно помещение).
    let mut used = HashSet::new();
    for r in doc.rooms.iter().filter(|r| shown(&r.layer)) {
        let Some(&i) = derived.room_regions.get(&r.id) else {
            continue;
        };
        if !used.insert(i) {
            continue;
        }
        let region = &derived.regions[i];
        let contours: Vec<Vec<DVec2>> = std::iter::once(to3(&region.contour))
            .chain(region.islands.iter().map(|p| to3(p)))
            .collect();
        // Пол и потолок — цветом материала отделки, если она задана.
        let finish = |surface: op_model::Surface| {
            doc.finishes
                .iter()
                .rev()
                .find(|f| f.room == r.id && f.surface == surface)
                .and_then(|f| crate::finish::material(doc, &f.material))
                .filter(|_| !palette.white)
                .map(|m| crate::finish::color(&m))
        };
        opaque.cap(
            &contours,
            0.0,
            true,
            finish(op_model::Surface::Floor).unwrap_or(palette.floor),
        );
        if options.ceiling && options.cut.is_none() {
            let height = r.ceiling_height.unwrap_or(default_height);
            opaque.cap(
                &contours,
                height,
                false,
                finish(op_model::Surface::Ceiling).unwrap_or(palette.ceiling),
            );
        }
    }

    Scene3D {
        opaque,
        glass,
        sun: sun(doc, options).and_then(|(_, light)| light),
    }
}

#[cfg(test)]
mod tests {
    use op_model::{
        Command, Element, Justification, LayerKind, Node, Opening, Room, Wall, view::project,
    };

    use super::*;
    use crate::protocol::SunTime;

    fn put(element: Element) -> Command {
        Command::Put {
            element,
            index: None,
        }
    }

    /// Комната 4000 × 3000 по осям, окно в верхней стене и дверь в нижней.
    fn room() -> Document {
        let mut doc = Document::new("Кв");
        let mut commands = Vec::new();
        for (i, (x, y)) in [(0.0, 0.0), (4000.0, 0.0), (4000.0, 3000.0), (0.0, 3000.0)]
            .iter()
            .enumerate()
        {
            commands.push(put(Element::Node(Node {
                level: None,
                id: format!("n{i}"),
                variant: None,
                x: *x,
                y: *y,
            })));
        }
        for i in 0..4 {
            commands.push(put(Element::Wall(Wall {
                id: format!("w{i}"),
                layer: LayerKind::Walls.id().into(),
                variant: None,
                a: format!("n{i}"),
                b: format!("n{}", (i + 1) % 4),
                thickness: 200.0,
                justification: Justification::Center,
                height: None,
                material: None,
            })));
        }
        for (id, wall, kind, width, height, sill) in [
            ("win", "w0", OpeningKind::Window, 1200.0, 1400.0, 850.0),
            ("door", "w2", OpeningKind::Door, 800.0, 2100.0, 0.0),
        ] {
            commands.push(put(Element::Opening(Opening {
                id: id.into(),
                layer: LayerKind::Openings.id().into(),
                variant: None,
                wall: wall.into(),
                kind,
                offset: 2000.0,
                width,
                height,
                sill,
                hinge_at_end: false,
                swing: Side::Left,
            })));
        }
        commands.push(put(Element::Room(Room {
            level: None,
            id: "r".into(),
            layer: LayerKind::Rooms.id().into(),
            variant: None,
            name: "Комната".into(),
            x: 2000.0,
            y: 1500.0,
            ceiling_height: None,
        })));
        for c in commands {
            c.apply(&mut doc).unwrap();
        }
        doc
    }

    fn scene(doc: &Document, options: Options3d) -> Scene3D {
        let doc = project(doc, None);
        build(&doc, &Derived::new(&doc), &options, &Models::new())
    }

    #[test]
    fn walls_rise_to_the_ceiling_with_openings_cut() {
        let doc = room();
        let s = scene(&doc, Options3d::default());
        let (lo, hi) = s.opaque.bounds().unwrap();
        assert!((hi.z - 2700.0).abs() < 1e-3 && lo.z.abs() < 1e-3);
        // Габарит по наружным граням стен; Y плана — вниз, в сцене — вверх.
        assert!(
            (lo.x + 100.0).abs() < 1e-3 && (hi.x - 4100.0).abs() < 1e-3,
            "{lo} {hi}"
        );
        // Снизу (по плану) за стену выходит открытое наружу полотно двери; вырез
        // проёма шире стены на 1 мм с каждой стороны — перемычка повторяет его.
        assert!(
            (hi.y - 100.0).abs() < 1.5 && lo.y < -3100.0 + 1e-3,
            "{lo} {hi}"
        );
        assert!(!s.glass.is_empty(), "стекло окна");
        // Проём окна: на высоте 1500 над серединой окна стены нет, на 2400 — перемычка.
        let at = |x: f64, y: f64, z: f64| {
            s.opaque.vertices.iter().any(|v| {
                (f64::from(v.pos[0]) - x).abs() < 1.0
                    && (f64::from(v.pos[1]) - y).abs() < 102.0
                    && (f64::from(v.pos[2]) - z).abs() < 1.0
            })
        };
        assert!(at(1400.0, 0.0, 2250.0), "низ перемычки над окном");
        assert!(at(1400.0, 0.0, 850.0), "верх подоконной части");
        assert!(s.sun.is_none());
    }

    #[test]
    fn cut_lowers_walls_and_hides_ceilings() {
        let doc = room();
        let full = scene(&doc, Options3d::default());
        let cut = scene(
            &doc,
            Options3d {
                cut: Some(1200.0),
                ..Options3d::default()
            },
        );
        let (_, hi) = cut.opaque.bounds().unwrap();
        assert!((hi.z - 1200.0).abs() < 1e-3);
        // Тёмный срез сверху стен.
        assert!(cut.opaque.vertices.iter().any(|v| {
            (f64::from(v.pos[2]) - 1200.0).abs() < 1e-3 && v.normal[2] > 0.5 && v.color[0] < 0.3
        }));
        let ceilings = |s: &Scene3D| {
            s.opaque
                .vertices
                .iter()
                .filter(|v| v.normal[2] < -0.5)
                .count()
        };
        assert!(ceilings(&full) > 0 && ceilings(&cut) == 0);
        let no_ceiling = scene(
            &doc,
            Options3d {
                ceiling: false,
                ..Options3d::default()
            },
        );
        assert!(ceilings(&no_ceiling) < ceilings(&full));
    }

    #[test]
    fn white_model_and_sun() {
        let doc = room();
        let white = scene(
            &doc,
            Options3d {
                white: true,
                sun: Some(SunTime {
                    day: sun::day_of_year(6, 21),
                    hour: 13.0,
                }),
                ..Options3d::default()
            },
        );
        // Пол белой модели — серый, а не цвета дерева.
        let floor = white
            .opaque
            .vertices
            .iter()
            .find(|v| v.normal[2] > 0.5 && v.pos[2].abs() < 1e-3)
            .unwrap();
        assert!((floor.color[0] - floor.color[2]).abs() < 0.02);
        let sun = white.sun.unwrap();
        assert!(sun.direction.z > 0.5 && sun.intensity > 0.9);
        // Ночью солнце за горизонтом — света нет.
        let night = scene(
            &doc,
            Options3d {
                sun: Some(SunTime {
                    day: 172,
                    hour: 1.0,
                }),
                ..Options3d::default()
            },
        );
        assert!(night.sun.is_none());
    }
}
