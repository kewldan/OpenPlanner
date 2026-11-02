//! Синтетические сканы для тестов и e2e: поверхности комнат такими, какими
//! их видит LiDAR изнутри (только внутренние стороны стен), с проёмами,
//! мебелью и шумом. Свои данные — без чужих сканов.

use glam::{DVec2, DVec3};

use crate::io::Mesh;

/// Проём в стене комнаты.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hole {
    /// Сторона комнаты: 0 — верх (y0), 1 — право (x1), 2 — низ (y1), 3 — лево (x0).
    pub side: usize,
    /// Середина вдоль стороны от её начала (x0 или y0), мм.
    pub center: f64,
    /// Ширина, мм.
    pub width: f64,
    /// Низ от пола, мм.
    pub bottom: f64,
    /// Верх от пола, мм.
    pub top: f64,
}

/// Коробка мебели на полу.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Furniture {
    /// Угол на плане, мм.
    pub min: DVec2,
    /// Противоположный угол, мм.
    pub max: DVec2,
    /// Высота, мм.
    pub height: f64,
}

/// Прямоугольная комната (внутренние размеры).
#[derive(Debug, Clone, PartialEq)]
pub struct Room {
    /// Левый верхний угол, мм.
    pub min: DVec2,
    /// Правый нижний угол, мм.
    pub max: DVec2,
    /// Проёмы.
    pub holes: Vec<Hole>,
    /// Мебель.
    pub furniture: Vec<Furniture>,
}

/// Сцена: комнаты, высота, поворот на плане, шум.
#[derive(Debug, Clone, PartialEq)]
pub struct Scene {
    /// Комнаты.
    pub rooms: Vec<Room>,
    /// Высота потолка, мм.
    pub height: f64,
    /// Поворот плана, рад.
    pub rotation: f64,
    /// Амплитуда шума, мм.
    pub noise: f64,
}

/// Шаг сетки поверхностей, мм.
const STEP: f64 = 100.0;

struct Builder {
    mesh: Mesh,
    rotation: f64,
    noise: f64,
    seed: u64,
}

impl Builder {
    fn jitter(&mut self) -> f64 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        #[allow(clippy::cast_precision_loss)] // псевдослучайное в [-1, 1]
        let unit = (self.seed % 2001) as f64 / 1000.0 - 1.0;
        unit * self.noise
    }

    /// Вершина плана (x, y, высота) в координатах GLB (метры, ось Y вверх).
    fn vertex(&mut self, p: DVec3) -> u32 {
        let q = DVec2::from_angle(self.rotation).rotate(p.truncate());
        let n = (self.jitter(), self.jitter(), self.jitter());
        // План (x, y) = (Z, −X) файла, высота — Y файла.
        let v = DVec3::new(-(q.y + n.1), p.z + n.2, q.x + n.0) / 1000.0;
        self.mesh.positions.push(v);
        u32::try_from(self.mesh.positions.len() - 1).unwrap_or(u32::MAX)
    }

    fn quad(&mut self, a: DVec3, b: DVec3, c: DVec3, d: DVec3) {
        let ids = [a, b, c, d].map(|p| self.vertex(p));
        self.mesh.triangles.push([ids[0], ids[1], ids[2]]);
        self.mesh.triangles.push([ids[0], ids[2], ids[3]]);
    }

    /// Вертикальная поверхность от `a` до `b` (план) сеткой ячеек, кроме `skip`.
    /// Границы ячеек проходят и через `cuts` (края проёмов) — проёмы точные.
    fn wall(
        &mut self,
        a: DVec2,
        b: DVec2,
        z0: f64,
        z1: f64,
        cuts: &[(f64, f64)],
        skip: &dyn Fn(f64, f64) -> bool,
    ) {
        let len = a.distance(b);
        let dir = (b - a) / len;
        let steps = |lo: f64, hi: f64, extra: &mut dyn Iterator<Item = f64>| {
            let mut v: Vec<f64> = Vec::new();
            let mut x = lo;
            while x < hi - 1e-6 {
                v.push(x);
                x += STEP;
            }
            v.push(hi);
            v.extend(extra.filter(|e| *e > lo + 1e-6 && *e < hi - 1e-6));
            v.sort_by(f64::total_cmp);
            v.dedup_by(|x, y| (*x - *y).abs() < 1e-6);
            v
        };
        let ts = steps(0.0, len, &mut cuts.iter().flat_map(|(t, _)| [*t]));
        let zs = steps(z0, z1, &mut cuts.iter().flat_map(|(_, z)| [*z]));
        for tw in ts.windows(2) {
            for zw in zs.windows(2) {
                let (t, t1, z, zt) = (tw[0], tw[1], zw[0], zw[1]);
                if !skip(f64::midpoint(t, t1), f64::midpoint(z, zt)) {
                    let (p, q) = (a + dir * t, a + dir * t1);
                    self.quad(p.extend(z), q.extend(z), q.extend(zt), p.extend(zt));
                }
            }
        }
    }

    /// Горизонтальная поверхность (пол, потолок, крышка мебели) сеткой.
    fn plane(&mut self, min: DVec2, max: DVec2, z: f64, skip: &dyn Fn(DVec2) -> bool) {
        let mut x = min.x;
        while x < max.x - 1e-6 {
            let x1 = (x + STEP * 2.0).min(max.x);
            let mut y = min.y;
            while y < max.y - 1e-6 {
                let y1 = (y + STEP * 2.0).min(max.y);
                if !skip(DVec2::new(f64::midpoint(x, x1), f64::midpoint(y, y1))) {
                    self.quad(
                        DVec3::new(x, y, z),
                        DVec3::new(x1, y, z),
                        DVec3::new(x1, y1, z),
                        DVec3::new(x, y1, z),
                    );
                }
                y = y1;
            }
            x = x1;
        }
    }
}

fn inside(p: DVec2, f: &Furniture) -> bool {
    p.x > f.min.x && p.x < f.max.x && p.y > f.min.y && p.y < f.max.y
}

/// Строит сетку сцены (координаты GLB: метры, ось Y вверх).
#[must_use]
pub fn mesh(scene: &Scene) -> Mesh {
    let mut b = Builder {
        mesh: Mesh::default(),
        rotation: scene.rotation,
        noise: scene.noise,
        seed: 0x9E37_79B9_7F4A_7C15,
    };
    let h = scene.height;
    for room in &scene.rooms {
        let (lo, hi) = (room.min, room.max);
        let corners = [
            (DVec2::new(lo.x, lo.y), DVec2::new(hi.x, lo.y)),
            (DVec2::new(hi.x, lo.y), DVec2::new(hi.x, hi.y)),
            (DVec2::new(lo.x, hi.y), DVec2::new(hi.x, hi.y)),
            (DVec2::new(lo.x, lo.y), DVec2::new(lo.x, hi.y)),
        ];
        for (side, (a, c)) in corners.into_iter().enumerate() {
            let holes: Vec<Hole> = room
                .holes
                .iter()
                .copied()
                .filter(|h| h.side == side)
                .collect();
            // Стена за мебелью не видна сканеру.
            let hidden: Vec<Furniture> = room.furniture.clone();
            let dir = (c - a).normalize();
            let skip = move |t: f64, z: f64| {
                let p = a + dir * t;
                holes
                    .iter()
                    .any(|o| (t - o.center).abs() < o.width / 2.0 && z > o.bottom && z < o.top)
                    || hidden.iter().any(|f| {
                        z < f.height
                            && p.x > f.min.x - 5.0
                            && p.x < f.max.x + 5.0
                            && p.y > f.min.y - 5.0
                            && p.y < f.max.y + 5.0
                    })
            };
            let cuts: Vec<(f64, f64)> = room
                .holes
                .iter()
                .filter(|o| o.side == side)
                .flat_map(|o| {
                    [
                        (o.center - o.width / 2.0, o.bottom),
                        (o.center + o.width / 2.0, o.top),
                    ]
                })
                .chain(room.furniture.iter().flat_map(|f| {
                    let lo = (f.min - a).dot(dir).min((f.max - a).dot(dir));
                    let hi = (f.min - a).dot(dir).max((f.max - a).dot(dir));
                    [(lo, f.height), (hi, f.height)]
                }))
                .collect();
            b.wall(a, c, 0.0, h, &cuts, &skip);
        }
        let furniture = room.furniture.clone();
        b.plane(lo, hi, 0.0, &|p| furniture.iter().any(|f| inside(p, f)));
        b.plane(lo, hi, h, &|_| false);
        for f in &room.furniture {
            let (a, c) = (f.min, f.max);
            for (p, q) in [
                (DVec2::new(a.x, a.y), DVec2::new(c.x, a.y)),
                (DVec2::new(c.x, a.y), DVec2::new(c.x, c.y)),
                (DVec2::new(c.x, c.y), DVec2::new(a.x, c.y)),
                (DVec2::new(a.x, c.y), DVec2::new(a.x, a.y)),
            ] {
                // Сторона вплотную к стене комнаты не видна.
                let against = [lo.x, hi.x]
                    .iter()
                    .any(|x| (p.x - x).abs() < 1.0 && (q.x - x).abs() < 1.0)
                    || [lo.y, hi.y]
                        .iter()
                        .any(|y| (p.y - y).abs() < 1.0 && (q.y - y).abs() < 1.0);
                if !against {
                    b.wall(p, q, 0.0, f.height, &[], &|_, _| false);
                }
            }
            b.plane(a, c, f.height, &|_| false);
        }
    }
    b.mesh
}

/// Двухкомнатная квартира: комнаты 3500 × 4000 и 3380 × 4000 мм, перегородка
/// 120 мм с дверью, входная дверь, два окна, шкаф у стены.
#[must_use]
pub fn apartment(rotation: f64) -> Scene {
    Scene {
        rooms: vec![
            Room {
                min: DVec2::new(0.0, 0.0),
                max: DVec2::new(3500.0, 4000.0),
                holes: vec![
                    Hole {
                        side: 0,
                        center: 1750.0,
                        width: 1200.0,
                        bottom: 850.0,
                        top: 2250.0,
                    },
                    Hole {
                        side: 1,
                        center: 2000.0,
                        width: 800.0,
                        bottom: 0.0,
                        top: 2100.0,
                    },
                ],
                furniture: vec![Furniture {
                    min: DVec2::new(0.0, 1500.0),
                    max: DVec2::new(600.0, 3300.0),
                    height: 2000.0,
                }],
            },
            Room {
                min: DVec2::new(3620.0, 0.0),
                max: DVec2::new(7000.0, 4000.0),
                holes: vec![
                    Hole {
                        side: 0,
                        center: 1680.0,
                        width: 1200.0,
                        bottom: 850.0,
                        top: 2250.0,
                    },
                    Hole {
                        side: 3,
                        center: 2000.0,
                        width: 800.0,
                        bottom: 0.0,
                        top: 2100.0,
                    },
                    Hole {
                        side: 2,
                        center: 1400.0,
                        width: 900.0,
                        bottom: 0.0,
                        top: 2100.0,
                    },
                ],
                furniture: Vec::new(),
            },
        ],
        height: 2700.0,
        rotation,
        noise: 4.0,
    }
}
