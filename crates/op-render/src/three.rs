//! 3D-сцена: треугольные сетки с нормалями и цветом, камера, солнце.
//!
//! Сетки строятся на CPU (в движке) и не зависят от GPU. Координаты — мм:
//! X — вправо по плану, Y — вверх по плану (к «северу» листа), Z — вверх.
//! Ось Y плана направлена вниз, поэтому точка плана `(x, y)` переходит в `(x, -y)`.

use bytemuck::{Pod, Zeroable};
use glam::{DMat4, DVec2, DVec3, DVec4};
use lyon_tessellation::{
    BuffersBuilder, FillOptions, FillRule, FillTessellator, FillVertex, VertexBuffers, math::point,
    path::Path,
};

use crate::scene::Color;

/// Вершина сетки.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable, PartialEq)]
pub struct Vertex3 {
    /// Точка, мм.
    pub pos: [f32; 3],
    /// Нормаль (единичная).
    pub normal: [f32; 3],
    /// Цвет sRGB с альфой.
    pub color: [f32; 4],
}

/// Треугольная сетка. Лицевая сторона треугольника — против часовой стрелки
/// при взгляде с конца нормали.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Mesh {
    /// Вершины.
    pub vertices: Vec<Vertex3>,
    /// Индексы треугольников.
    pub indices: Vec<u32>,
}

#[allow(clippy::cast_possible_truncation)] // f32 достаточно для GPU
fn f32x3(v: DVec3) -> [f32; 3] {
    [v.x as f32, v.y as f32, v.z as f32]
}

impl Mesh {
    /// Нет треугольников.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }

    /// Число треугольников.
    #[must_use]
    pub fn triangles(&self) -> usize {
        self.indices.len() / 3
    }

    /// Габарит вершин: (мин, макс).
    #[must_use]
    pub fn bounds(&self) -> Option<(DVec3, DVec3)> {
        let mut it = self.vertices.iter().map(|v| {
            let [x, y, z] = v.pos;
            DVec3::new(f64::from(x), f64::from(y), f64::from(z))
        });
        let first = it.next()?;
        Some(it.fold((first, first), |(lo, hi), p| (lo.min(p), hi.max(p))))
    }

    /// Добавляет другую сетку.
    pub fn append(&mut self, other: &Self) {
        let base = u32::try_from(self.vertices.len()).unwrap_or(u32::MAX);
        self.vertices.extend_from_slice(&other.vertices);
        self.indices.extend(other.indices.iter().map(|i| i + base));
    }

    /// Треугольники с общей нормалью. Порядок обхода исправляется по нормали.
    fn polygon(&mut self, points: &[DVec3], triangles: &[[u32; 3]], normal: DVec3, color: Color) {
        let base = u32::try_from(self.vertices.len()).unwrap_or(u32::MAX);
        let n = f32x3(normal);
        self.vertices.extend(points.iter().map(|p| Vertex3 {
            pos: f32x3(*p),
            normal: n,
            color: color.0,
        }));
        for &[a, b, c] in triangles {
            let (pa, pb, pc) = (points[a as usize], points[b as usize], points[c as usize]);
            let facing = (pb - pa).cross(pc - pa).dot(normal);
            if facing.abs() < 1e-12 {
                continue;
            }
            if facing > 0.0 {
                self.indices.extend([base + a, base + b, base + c]);
            } else {
                self.indices.extend([base + a, base + c, base + b]);
            }
        }
    }

    /// Четырёхугольник `a b c d` с нормалью `normal`.
    pub fn quad(&mut self, corners: [DVec3; 4], normal: DVec3, color: Color) {
        self.polygon(&corners, &[[0, 1, 2], [0, 2, 3]], normal, color);
    }

    /// Горизонтальная заливка контуров (правило чёт-нечет) на высоте `z`;
    /// `up` — лицом вверх, иначе вниз.
    pub fn cap(&mut self, rings: &[Vec<DVec2>], z: f64, up: bool, color: Color) {
        let Some((points, triangles)) = triangulate(rings) else {
            return;
        };
        let points: Vec<DVec3> = points.iter().map(|p| p.extend(z)).collect();
        let normal = if up { DVec3::Z } else { DVec3::NEG_Z };
        self.polygon(&points, &triangles, normal, color);
    }

    /// Призма: контуры (чёт-нечет) от `z0` до `z1`. Боковые грани смотрят
    /// из тела наружу; `top` и `bottom` — закрыть верх и низ.
    pub fn prism(
        &mut self,
        rings: &[Vec<DVec2>],
        z0: f64,
        z1: f64,
        color: Color,
        top: Option<Color>,
        bottom: bool,
    ) {
        if z1 - z0 < 1e-6 {
            return;
        }
        for ring in rings {
            for i in 0..ring.len() {
                let (a, b) = (ring[i], ring[(i + 1) % ring.len()]);
                let edge = b - a;
                let len = edge.length();
                if len < 1e-6 {
                    continue;
                }
                let mut n = DVec2::new(edge.y, -edge.x) / len;
                // Наружу — туда, где не тело призмы.
                if inside(rings, a.midpoint(b) + n * 0.5) {
                    n = -n;
                }
                self.quad(
                    [a.extend(z0), b.extend(z0), b.extend(z1), a.extend(z1)],
                    n.extend(0.0),
                    color,
                );
            }
        }
        if let Some(top) = top {
            self.cap(rings, z1, true, top);
        }
        if bottom {
            self.cap(rings, z0, false, color);
        }
    }

    /// Наклонный прямоугольный брусок: основание `rect` (4 точки плана 3D-осей),
    /// по высоте от `z0` до `z1`.
    pub fn block(&mut self, rect: [DVec2; 4], z0: f64, z1: f64, color: Color) {
        self.prism(&[rect.to_vec()], z0, z1, color, Some(color), true);
    }
}

/// Внутри ли точка контуров (правило чёт-нечет).
#[must_use]
pub fn inside(rings: &[Vec<DVec2>], p: DVec2) -> bool {
    let mut odd = false;
    for ring in rings {
        for i in 0..ring.len() {
            let (a, b) = (ring[i], ring[(i + 1) % ring.len()]);
            if (a.y > p.y) != (b.y > p.y) && p.x < a.x + (p.y - a.y) / (b.y - a.y) * (b.x - a.x) {
                odd = !odd;
            }
        }
    }
    odd
}

/// Треугольники контуров (правило чёт-нечет): точки и индексы.
fn triangulate(rings: &[Vec<DVec2>]) -> Option<(Vec<DVec2>, Vec<[u32; 3]>)> {
    let mut builder = Path::builder();
    let mut any = false;
    for ring in rings.iter().filter(|r| r.len() >= 3) {
        #[allow(clippy::cast_possible_truncation)] // f32 достаточно для тесселяции
        let pt = |p: DVec2| point(p.x as f32, p.y as f32);
        builder.begin(pt(ring[0]));
        for p in &ring[1..] {
            builder.line_to(pt(*p));
        }
        builder.end(true);
        any = true;
    }
    if !any {
        return None;
    }
    let path = builder.build();
    let mut buffers: VertexBuffers<DVec2, u32> = VertexBuffers::new();
    FillTessellator::new()
        .tessellate_path(
            &path,
            &FillOptions::default()
                .with_fill_rule(FillRule::EvenOdd)
                .with_tolerance(0.5),
            &mut BuffersBuilder::new(&mut buffers, |v: FillVertex<'_>| {
                let p = v.position();
                DVec2::new(f64::from(p.x), f64::from(p.y))
            }),
        )
        .ok()?;
    let triangles = buffers.indices.as_chunks::<3>().0.to_vec();
    Some((buffers.vertices, triangles))
}

/// Солнце: направление на солнце (единичное) и сила света.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sun {
    /// Направление из сцены на солнце.
    pub direction: DVec3,
    /// Сила прямого света, 0–1.
    pub intensity: f64,
}

/// 3D-сцена.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Scene3D {
    /// Непрозрачные поверхности.
    pub opaque: Mesh,
    /// Полупрозрачные (стёкла) — рисуются после непрозрачных.
    pub glass: Mesh,
    /// Солнце с тенями; без него — рассеянный свет «облачного дня».
    pub sun: Option<Sun>,
}

/// Перспективная камера.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camera3D {
    /// Глаз, мм.
    pub eye: DVec3,
    /// Точка, куда смотрит камера, мм.
    pub target: DVec3,
    /// Вертикальный угол обзора, радианы.
    pub fov_y: f64,
}

impl Camera3D {
    /// Ближняя плоскость отсечения, мм.
    pub const NEAR: f64 = 50.0;
    /// Дальняя плоскость отсечения, мм.
    pub const FAR: f64 = 500_000.0;

    /// Матрица вид × проекция для соотношения сторон `aspect` (ширина / высота).
    #[must_use]
    pub fn view_proj(&self, aspect: f64) -> DMat4 {
        let forward = (self.target - self.eye).normalize_or(DVec3::Y);
        // Взгляд строго вниз или вверх: «вверх» экрана — вдоль Y.
        let up = if forward.cross(DVec3::Z).length() < 1e-6 {
            DVec3::Y
        } else {
            DVec3::Z
        };
        let view = look_at_rh(self.eye, self.eye + forward, up);
        let proj = perspective_rh(self.fov_y, aspect.max(1e-3), Self::NEAR, Self::FAR);
        proj * view
    }
}

/// Матрица вида и ортографической проекции с солнца, охватывающая габарит сцены.
#[must_use]
pub fn sun_view_proj(sun: DVec3, bounds: (DVec3, DVec3)) -> DMat4 {
    let (lo, hi) = bounds;
    let center = lo.midpoint(hi);
    let radius = (hi - lo).length() / 2.0 + 100.0;
    let dir = sun.normalize_or(DVec3::Z);
    let up = if dir.cross(DVec3::Z).length() < 1e-6 {
        DVec3::Y
    } else {
        DVec3::Z
    };
    let eye = center + dir * radius * 2.0;
    let view = look_at_rh(eye, center, up);
    let proj = orthographic_rh(-radius, radius, -radius, radius, radius * 0.5, radius * 3.5);
    proj * view
}

/// Матрица вида: правая система, камера смотрит вдоль −Z.
fn look_at_rh(eye: DVec3, center: DVec3, up: DVec3) -> DMat4 {
    let f = (center - eye).normalize();
    let s = f.cross(up).normalize();
    let u = s.cross(f);
    DMat4::from_cols(
        DVec4::new(s.x, u.x, -f.x, 0.0),
        DVec4::new(s.y, u.y, -f.y, 0.0),
        DVec4::new(s.z, u.z, -f.z, 0.0),
        DVec4::new(-s.dot(eye), -u.dot(eye), f.dot(eye), 1.0),
    )
}

/// Перспективная проекция с глубиной 0–1 (как в wgpu).
fn perspective_rh(fov_y: f64, aspect: f64, near: f64, far: f64) -> DMat4 {
    let h = 1.0 / (fov_y / 2.0).tan();
    let r = far / (near - far);
    DMat4::from_cols(
        DVec4::new(h / aspect, 0.0, 0.0, 0.0),
        DVec4::new(0.0, h, 0.0, 0.0),
        DVec4::new(0.0, 0.0, r, -1.0),
        DVec4::new(0.0, 0.0, r * near, 0.0),
    )
}

/// Ортографическая проекция с глубиной 0–1.
fn orthographic_rh(left: f64, right: f64, bottom: f64, top: f64, near: f64, far: f64) -> DMat4 {
    let (w, h, r) = (
        1.0 / (right - left),
        1.0 / (top - bottom),
        1.0 / (near - far),
    );
    DMat4::from_cols(
        DVec4::new(2.0 * w, 0.0, 0.0, 0.0),
        DVec4::new(0.0, 2.0 * h, 0.0, 0.0),
        DVec4::new(0.0, 0.0, r, 0.0),
        DVec4::new(-(left + right) * w, -(top + bottom) * h, r * near, 1.0),
    )
}

/// Точка плана (Y вниз) в координатах 3D-сцены.
#[must_use]
pub fn from_plan(p: DVec2) -> DVec2 {
    DVec2::new(p.x, -p.y)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(lo: f64, hi: f64) -> Vec<DVec2> {
        vec![
            DVec2::new(lo, lo),
            DVec2::new(hi, lo),
            DVec2::new(hi, hi),
            DVec2::new(lo, hi),
        ]
    }

    fn face_normals_point_out(mesh: &Mesh, center: DVec3) {
        for t in mesh.indices.as_chunks::<3>().0 {
            let p = |i: u32| {
                let [x, y, z] = mesh.vertices[i as usize].pos;
                DVec3::new(f64::from(x), f64::from(y), f64::from(z))
            };
            let (a, b, c) = (p(t[0]), p(t[1]), p(t[2]));
            let n = (b - a).cross(c - a);
            let mid = (a + b + c) / 3.0;
            assert!(
                n.dot(mid - center) > 0.0,
                "треугольник смотрит внутрь: {a} {b} {c}"
            );
        }
    }

    #[test]
    fn prism_is_closed_and_faces_out() {
        let mut mesh = Mesh::default();
        let c = Color([1.0; 4]);
        mesh.prism(&[square(0.0, 100.0)], 0.0, 50.0, c, Some(c), true);
        // 4 стороны × 2 + верх 2 + низ 2.
        assert_eq!(mesh.triangles(), 12);
        face_normals_point_out(&mesh, DVec3::new(50.0, 50.0, 25.0));
        let (lo, hi) = mesh.bounds().unwrap();
        assert_eq!((lo.z, hi.z), (0.0, 50.0));
    }

    #[test]
    fn ring_with_hole_has_inner_walls_facing_the_hole() {
        let mut mesh = Mesh::default();
        let c = Color([1.0; 4]);
        mesh.prism(
            &[square(0.0, 300.0), square(100.0, 200.0)],
            0.0,
            10.0,
            c,
            None,
            false,
        );
        assert_eq!(mesh.triangles(), 16);
        // Нормали внутренних стен смотрят в отверстие (к центру).
        let inner: Vec<_> = mesh
            .vertices
            .iter()
            .filter(|v| v.pos[0] > 99.0 && v.pos[0] < 201.0 && v.pos[1] > 99.0 && v.pos[1] < 201.0)
            .collect();
        for v in inner {
            let to_center = DVec2::new(150.0 - f64::from(v.pos[0]), 150.0 - f64::from(v.pos[1]));
            let n = DVec2::new(f64::from(v.normal[0]), f64::from(v.normal[1]));
            assert!(n.dot(to_center) > 0.0);
        }
    }

    #[test]
    fn camera_projects_target_to_center() {
        let camera = Camera3D {
            eye: DVec3::new(0.0, -5000.0, 3000.0),
            target: DVec3::new(0.0, 0.0, 0.0),
            fov_y: 0.8,
        };
        let m = camera.view_proj(1.5);
        let p = m.project_point3(camera.target);
        assert!(p.x.abs() < 1e-9 && p.y.abs() < 1e-9 && p.z > 0.0 && p.z < 1.0);
        // Точка справа (по +X) — справа на экране: план не зеркалится.
        assert!(m.project_point3(DVec3::new(1000.0, 0.0, 0.0)).x > 0.0);
        // Строго сверху тоже без зеркала.
        let top = Camera3D {
            eye: DVec3::new(0.0, 0.0, 9000.0),
            ..camera
        };
        let m = top.view_proj(1.0);
        assert!(m.project_point3(DVec3::new(1000.0, 0.0, 0.0)).x > 0.0);
        assert!(m.project_point3(DVec3::new(0.0, 1000.0, 0.0)).y > 0.0);
    }
}
