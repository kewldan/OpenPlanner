//! Фотореалистичный кадр трассировкой путей на CPU: та же сцена, что у 3D-вида,
//! диффузные поверхности цвета материала, солнце с мягкой тенью и небо,
//! несколько отскоков света, автоэкспозиция и тональная кривая.
//!
//! Работает без GPU и без потоков — в WASM в отдельном воркере, кадр копится
//! проходами: после каждого прохода можно показать прогресс или прервать.

use glam::{DVec3, Vec3};

use crate::three::{Camera3D, Mesh, Scene3D, Sun};

/// Треугольник сцены.
#[derive(Debug, Clone, Copy)]
struct Tri {
    a: Vec3,
    e1: Vec3,
    e2: Vec3,
    normal: Vec3,
    albedo: Vec3,
}

#[derive(Debug, Clone, Copy)]
struct Node {
    lo: Vec3,
    hi: Vec3,
    /// Лист: первый треугольник; узел: левый ребёнок (правый — `left + 1`).
    start: u32,
    /// Треугольников в листе (0 — внутренний узел).
    count: u32,
}

/// Сцена для трассировки: треугольники и BVH.
#[derive(Debug, Clone, Default)]
pub struct TraceScene {
    tris: Vec<Tri>,
    nodes: Vec<Node>,
    sun: Option<(Vec3, f32)>,
}

/// Луч до ближайшего пересечения: расстояние и треугольник.
type Hit = Option<(f32, usize)>;

fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb(c: f32) -> f32 {
    if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

impl TraceScene {
    /// Сцена из 3D-модели: непрозрачные поверхности (стёкла пропускают свет).
    #[must_use]
    pub fn new(scene: &Scene3D) -> Self {
        let mut tris = Vec::new();
        add_mesh(&mut tris, &scene.opaque);
        let mut out = Self {
            nodes: Vec::new(),
            sun: scene.sun.map(
                |Sun {
                     direction,
                     intensity,
                 }| {
                    #[allow(clippy::cast_possible_truncation)] // f32 достаточно для света
                    (direction.as_vec3().normalize_or_zero(), intensity as f32)
                },
            ),
            tris,
        };
        out.build();
        out
    }

    /// Треугольников в сцене.
    #[must_use]
    pub fn len(&self) -> usize {
        self.tris.len()
    }

    /// Пустая сцена.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tris.is_empty()
    }

    fn bounds(&self, range: std::ops::Range<usize>) -> (Vec3, Vec3) {
        self.tris[range].iter().fold(
            (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY)),
            |(lo, hi), t| {
                let (b, c) = (t.a + t.e1, t.a + t.e2);
                (lo.min(t.a).min(b).min(c), hi.max(t.a).max(b).max(c))
            },
        )
    }

    /// BVH делением по середине самой длинной оси центров.
    fn build(&mut self) {
        if self.tris.is_empty() {
            return;
        }
        let (lo, hi) = self.bounds(0..self.tris.len());
        self.nodes.push(Node {
            lo,
            hi,
            start: 0,
            count: u32::try_from(self.tris.len()).unwrap_or(u32::MAX),
        });
        let mut stack = vec![0usize];
        while let Some(i) = stack.pop() {
            let node = self.nodes[i];
            if node.count <= 4 {
                continue;
            }
            let (start, count) = (node.start as usize, node.count as usize);
            let centers = |t: &Tri| t.a + (t.e1 + t.e2) / 3.0;
            let (clo, chi) = self.tris[start..start + count].iter().fold(
                (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY)),
                |(lo, hi), t| (lo.min(centers(t)), hi.max(centers(t))),
            );
            let extent = chi - clo;
            let axis = if extent.x >= extent.y && extent.x >= extent.z {
                0
            } else if extent.y >= extent.z {
                1
            } else {
                2
            };
            self.tris[start..start + count]
                .sort_unstable_by(|a, b| centers(a)[axis].total_cmp(&centers(b)[axis]));
            let mid = count / 2;
            let left = self.nodes.len();
            for (s, c) in [(start, mid), (start + mid, count - mid)] {
                let (lo, hi) = self.bounds(s..s + c);
                self.nodes.push(Node {
                    lo,
                    hi,
                    start: u32::try_from(s).unwrap_or(u32::MAX),
                    count: u32::try_from(c).unwrap_or(u32::MAX),
                });
            }
            self.nodes[i] = Node {
                start: u32::try_from(left).unwrap_or(u32::MAX),
                count: 0,
                ..node
            };
            stack.push(left);
            stack.push(left + 1);
        }
    }

    fn slab(lo: Vec3, hi: Vec3, origin: Vec3, inv: Vec3, max: f32) -> bool {
        let t0 = (lo - origin) * inv;
        let t1 = (hi - origin) * inv;
        let near = t0.min(t1).max_element().max(0.0);
        let far = t0.max(t1).min_element().min(max);
        near <= far
    }

    /// Ближайшее пересечение луча (Мёллер — Трумбор) ближе `max`. Лучи камеры и
    /// отскоков видят грани только с лицевой стороны, как 3D-вид (потолок снаружи
    /// не виден); теневые лучи (`shadow`) задерживаются гранями с обеих сторон.
    fn hit(&self, origin: Vec3, dir: Vec3, max: f32, shadow: bool) -> Hit {
        if self.nodes.is_empty() {
            return None;
        }
        let inv = dir.recip();
        let mut best: Hit = None;
        let mut limit = max;
        let mut stack = [0u32; 64];
        let mut top = 1;
        while top > 0 {
            top -= 1;
            let node = self.nodes[stack[top] as usize];
            if !Self::slab(node.lo, node.hi, origin, inv, limit) {
                continue;
            }
            if node.count == 0 {
                if top + 2 <= stack.len() {
                    stack[top] = node.start;
                    stack[top + 1] = node.start + 1;
                    top += 2;
                }
                continue;
            }
            let start = node.start as usize;
            for i in start..start + node.count as usize {
                let t = &self.tris[i];
                if !shadow && t.normal.dot(dir) > 0.0 {
                    continue;
                }
                let p = dir.cross(t.e2);
                let det = t.e1.dot(p);
                if det.abs() < 1e-9 {
                    continue;
                }
                let f = 1.0 / det;
                let s = origin - t.a;
                let u = f * s.dot(p);
                if !(0.0..=1.0).contains(&u) {
                    continue;
                }
                let q = s.cross(t.e1);
                let v = f * dir.dot(q);
                if v < 0.0 || u + v > 1.0 {
                    continue;
                }
                let d = f * t.e2.dot(q);
                if d > 0.5 && d < limit {
                    limit = d;
                    best = Some((d, i));
                }
            }
        }
        best
    }
}

fn add_mesh(tris: &mut Vec<Tri>, mesh: &Mesh) {
    for t in mesh.indices.as_chunks::<3>().0 {
        let v = |i: u32| mesh.vertices.get(i as usize);
        let (Some(a), Some(b), Some(c)) = (v(t[0]), v(t[1]), v(t[2])) else {
            continue;
        };
        let (pa, pb, pc) = (Vec3::from(a.pos), Vec3::from(b.pos), Vec3::from(c.pos));
        let normal = (pb - pa).cross(pc - pa).normalize_or_zero();
        if normal == Vec3::ZERO {
            continue;
        }
        let albedo = Vec3::new(
            srgb_to_linear(a.color[0]),
            srgb_to_linear(a.color[1]),
            srgb_to_linear(a.color[2]),
        ) * 0.85;
        tris.push(Tri {
            a: pa,
            e1: pb - pa,
            e2: pc - pa,
            normal,
            albedo,
        });
    }
}

/// Генератор случайных чисел xorshift32.
struct Rng(u32);

impl Rng {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        #[allow(clippy::cast_precision_loss)] // 24 бита в мантиссу
        let v = (self.0 >> 8) as f32 / (1u32 << 24) as f32;
        v
    }
}

/// Направление по косинусу вокруг нормали.
fn cosine(n: Vec3, rng: &mut Rng) -> Vec3 {
    let (r1, r2) = (rng.next(), rng.next());
    let phi = std::f32::consts::TAU * r1;
    let r = r2.sqrt();
    let local = Vec3::new(r * phi.cos(), r * phi.sin(), (1.0 - r2).sqrt());
    let t = if n.z.abs() < 0.9 { Vec3::Z } else { Vec3::X };
    let u = t.cross(n).normalize();
    let v = n.cross(u);
    (u * local.x + v * local.y + n * local.z).normalize()
}

/// Небо: светлее к зениту, тёплый оттенок у горизонта.
fn sky(dir: Vec3) -> Vec3 {
    let up = dir.z.max(0.0);
    let horizon = Vec3::new(0.85, 0.85, 0.82);
    let zenith = Vec3::new(0.45, 0.62, 0.95);
    let ground = Vec3::new(0.25, 0.24, 0.22);
    if dir.z < 0.0 {
        ground
    } else {
        horizon.lerp(zenith, up.sqrt())
    }
}

/// Яркость, которую приносит путь из точки по направлению (`depth` отскоков осталось).
fn radiance(scene: &TraceScene, origin: Vec3, dir: Vec3, depth: u32, rng: &mut Rng) -> Vec3 {
    let mut throughput = Vec3::ONE;
    let mut color = Vec3::ZERO;
    let (mut o, mut d) = (origin, dir);
    for bounce in 0..=depth {
        let Some((t, i)) = scene.hit(o, d, 1e7, false) else {
            color += throughput * sky(d) * if bounce == 0 { 1.0 } else { 1.6 };
            break;
        };
        let tri = &scene.tris[i];
        let n = if tri.normal.dot(d) < 0.0 {
            tri.normal
        } else {
            -tri.normal
        };
        let p = o + d * t + n * 1.0;
        // Прямое солнце с мягкой тенью (дрожание направления).
        if let Some((sun, power)) = scene.sun {
            let jitter = Vec3::new(rng.next() - 0.5, rng.next() - 0.5, rng.next() - 0.5) * 0.02;
            let l = (sun + jitter).normalize();
            let cos = n.dot(l);
            if cos > 0.0 && scene.hit(p, l, 1e7, true).is_none() {
                color += throughput * tri.albedo * Vec3::new(1.0, 0.95, 0.85) * (3.0 * power * cos);
            }
        }
        throughput *= tri.albedo;
        if throughput.max_element() < 0.02 {
            break;
        }
        o = p;
        d = cosine(n, rng);
    }
    color
}

/// Кадр трассировки: накопленная яркость пикселей.
#[derive(Debug, Clone)]
pub struct Frame {
    /// Ширина, пиксели.
    pub width: u32,
    /// Высота, пиксели.
    pub height: u32,
    sum: Vec<Vec3>,
    /// Проходов накоплено.
    pub passes: u32,
}

impl Frame {
    /// Пустой кадр.
    #[must_use]
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            sum: vec![Vec3::ZERO; width as usize * height as usize],
            passes: 0,
        }
    }

    /// Один проход: по лучу на пиксель со случайным сдвигом внутри пикселя.
    pub fn pass(&mut self, scene: &TraceScene, camera: &Camera3D, bounces: u32) {
        let eye = camera.eye.as_vec3();
        let forward = (camera.target - camera.eye).as_vec3().normalize_or(Vec3::Y);
        let right = forward.cross(Vec3::Z).normalize_or(Vec3::X);
        let up = right.cross(forward);
        #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)] // f32 хватает
        let (h, aspect) = (
            (camera.fov_y / 2.0).tan() as f32,
            self.width as f32 / self.height.max(1) as f32,
        );
        let mut rng = Rng(0x9E37_79B9 ^ (self.passes.wrapping_mul(0x85EB_CA6B) | 1));
        for y in 0..self.height {
            for x in 0..self.width {
                #[allow(clippy::cast_precision_loss)] // координаты пикселей малы
                let (sx, sy) = (
                    ((x as f32 + rng.next()) / self.width as f32) * 2.0 - 1.0,
                    1.0 - ((y as f32 + rng.next()) / self.height as f32) * 2.0,
                );
                let dir = (forward + right * sx * h * aspect + up * sy * h).normalize();
                let c = radiance(scene, eye, dir, bounces, &mut rng);
                self.sum[(y * self.width + x) as usize] += c;
            }
        }
        self.passes += 1;
    }

    /// RGBA8 с автоэкспозицией по средней яркости и тональной кривой ACES.
    #[must_use]
    pub fn rgba(&self) -> Vec<u8> {
        #[allow(clippy::cast_precision_loss)] // проходов немного
        let n = self.passes.max(1) as f32;
        let luminance = |c: Vec3| c.dot(Vec3::new(0.2126, 0.7152, 0.0722));
        // Экспозиция по среднему логарифму яркости (ключ 0,18).
        #[allow(clippy::cast_precision_loss)] // пикселей — миллионы
        let log_avg = (self
            .sum
            .iter()
            .map(|c| (luminance(*c / n) + 1e-4).ln())
            .sum::<f32>()
            / self.sum.len().max(1) as f32)
            .exp();
        let exposure = 0.18 / log_avg.max(1e-4);
        let aces =
            |x: f32| ((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14)).clamp(0.0, 1.0);
        let mut out = Vec::with_capacity(self.sum.len() * 4);
        for c in &self.sum {
            let c = *c / n * exposure;
            for v in [c.x, c.y, c.z] {
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // 0–255
                out.push((linear_to_srgb(aces(v)) * 255.0).round() as u8);
            }
            out.push(255);
        }
        out
    }
}

/// Сцена и камера одним блоком байт — для передачи в воркер рендера.
/// Формат: камера (7 f32), солнце (4 f32: направление, сила; сила 0 — нет),
/// число треугольников (u32), затем по 12 f32 на треугольник (3 точки и альбедо).
#[must_use]
pub fn encode(scene: &Scene3D, camera: &Camera3D) -> Vec<u8> {
    let trace = TraceScene::new(scene);
    let mut out = Vec::with_capacity(48 + trace.tris.len() * 48);
    let mut put = |v: f32| out.extend_from_slice(&v.to_le_bytes());
    #[allow(clippy::cast_possible_truncation)] // f32 достаточно для сцены квартиры
    for v in [
        camera.eye.x as f32,
        camera.eye.y as f32,
        camera.eye.z as f32,
        camera.target.x as f32,
        camera.target.y as f32,
        camera.target.z as f32,
        camera.fov_y as f32,
    ] {
        put(v);
    }
    let (dir, power) = trace.sun.unwrap_or((Vec3::Z, 0.0));
    for v in [dir.x, dir.y, dir.z, power] {
        put(v);
    }
    out.extend_from_slice(
        &u32::try_from(trace.tris.len())
            .unwrap_or(u32::MAX)
            .to_le_bytes(),
    );
    for t in &trace.tris {
        let (b, c) = (t.a + t.e1, t.a + t.e2);
        for v in [t.a, b, c, t.albedo] {
            for k in v.to_array() {
                out.extend_from_slice(&k.to_le_bytes());
            }
        }
    }
    out
}

/// Сцена и камера из [`encode`].
#[must_use]
pub fn decode(bytes: &[u8]) -> Option<(TraceScene, Camera3D)> {
    let f = |i: usize| -> Option<f32> {
        Some(f32::from_le_bytes(
            bytes.get(i * 4..i * 4 + 4)?.try_into().ok()?,
        ))
    };
    let camera = Camera3D {
        eye: DVec3::new(f64::from(f(0)?), f64::from(f(1)?), f64::from(f(2)?)),
        target: DVec3::new(f64::from(f(3)?), f64::from(f(4)?), f64::from(f(5)?)),
        fov_y: f64::from(f(6)?),
    };
    let (dir, power) = (Vec3::new(f(7)?, f(8)?, f(9)?), f(10)?);
    let count = u32::from_le_bytes(bytes.get(44..48)?.try_into().ok()?) as usize;
    if bytes.len() < 48 + count * 48 {
        return None;
    }
    let mut tris = Vec::with_capacity(count);
    for i in 0..count {
        let base = 12 + i * 12;
        let v = |k: usize| -> Option<Vec3> {
            Some(Vec3::new(
                f(base + k * 3)?,
                f(base + k * 3 + 1)?,
                f(base + k * 3 + 2)?,
            ))
        };
        let (a, b, c, albedo) = (v(0)?, v(1)?, v(2)?, v(3)?);
        let normal = (b - a).cross(c - a).normalize_or_zero();
        tris.push(Tri {
            a,
            e1: b - a,
            e2: c - a,
            normal,
            albedo,
        });
    }
    let mut scene = TraceScene {
        tris,
        nodes: Vec::new(),
        sun: (power > 0.0).then_some((dir, power)),
    };
    scene.build();
    Some((scene, camera))
}

/// Кадр целиком: `passes` проходов, `progress(pass)` после каждого; `false` — прервать.
pub fn render(
    scene: &TraceScene,
    camera: &Camera3D,
    width: u32,
    height: u32,
    passes: u32,
    mut progress: impl FnMut(u32) -> bool,
) -> Frame {
    let mut frame = Frame::new(width, height);
    for i in 0..passes {
        frame.pass(scene, camera, 3);
        if !progress(i + 1) {
            break;
        }
    }
    frame
}

/// Камера по точкам в метрах сцены — для тестов и UI.
#[must_use]
pub fn camera(eye: DVec3, target: DVec3, fov_deg: f64) -> Camera3D {
    Camera3D {
        eye,
        target,
        fov_y: fov_deg.to_radians(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::Color;

    /// Пол 4 × 4 м и стена-брусок: освещённый пол светлее затенённого.
    fn scene(sun: bool) -> Scene3D {
        let mut opaque = Mesh::default();
        let c = |x: f64, y: f64| glam::DVec2::new(x, y);
        opaque.block(
            [
                c(0.0, 0.0),
                c(4000.0, 0.0),
                c(4000.0, 4000.0),
                c(0.0, 4000.0),
            ],
            -100.0,
            0.0,
            Color::hex(0xCCCCCC, 1.0),
        );
        opaque.block(
            [
                c(1800.0, 1800.0),
                c(2200.0, 1800.0),
                c(2200.0, 2200.0),
                c(1800.0, 2200.0),
            ],
            0.0,
            2000.0,
            Color::hex(0xB04030, 1.0),
        );
        Scene3D {
            opaque,
            glass: Mesh::default(),
            sun: sun.then_some(Sun {
                direction: DVec3::new(0.6, 0.0, 0.8),
                intensity: 1.0,
            }),
        }
    }

    #[test]
    fn bvh_finds_the_same_hits_as_brute_force() {
        let s = TraceScene::new(&scene(false));
        assert!(s.len() >= 24);
        let origin = Vec3::new(500.0, 500.0, 3000.0);
        let dir = (Vec3::new(2000.0, 2000.0, 1000.0) - origin).normalize();
        let (t, i) = s.hit(origin, dir, 1e7, false).unwrap();
        // Луч сверху в брусок: попадание в красный материал.
        assert!(s.tris[i].albedo.x > s.tris[i].albedo.z);
        assert!(t > 0.0);
        assert!(s.hit(origin, Vec3::Z, 1e7, false).is_none());
        // Изнутри плиты пола её верхняя грань не видна, но тень она даёт.
        let below = Vec3::new(2000.0, 500.0, -50.0);
        assert!(s.hit(below, Vec3::Z, 1e7, false).is_none());
        assert!(s.hit(below, Vec3::Z, 1e7, true).is_some());
    }

    #[test]
    fn scene_survives_encoding() {
        let cam = camera(DVec3::new(1.0, 2.0, 3.0), DVec3::new(4.0, 5.0, 6.0), 50.0);
        let bytes = encode(&scene(true), &cam);
        let (back, c) = decode(&bytes).unwrap();
        assert_eq!(back.len(), TraceScene::new(&scene(true)).len());
        assert!((c.eye.y - 2.0).abs() < 1e-6 && back.sun.is_some());
        assert!(decode(&bytes[..20]).is_none());
    }

    #[test]
    fn shadow_side_is_darker_and_frame_is_stable() {
        let s = TraceScene::new(&scene(true));
        let cam = camera(
            DVec3::new(2000.0, -3000.0, 3000.0),
            DVec3::new(2000.0, 2000.0, 0.0),
            60.0,
        );
        let frame = render(&s, &cam, 48, 32, 4, |_| true);
        assert_eq!(frame.passes, 4);
        let rgba = frame.rgba();
        assert_eq!(rgba.len(), 48 * 32 * 4);
        assert!(rgba.chunks(4).any(|p| p[0] > 40), "кадр не чёрный");
        // Прерывание после первого прохода.
        let one = render(&s, &cam, 8, 8, 10, |_| false);
        assert_eq!(one.passes, 1);
    }
}
