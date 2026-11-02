//! Выборка поверхности: точки плана (мм, ось Z вверх) с нормалями и весом
//! (площадью, которую представляет точка, мм²).

use std::collections::HashMap;

use glam::{DMat3, DVec3};

use crate::io::{Loaded, Up};

/// Точка поверхности.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sample {
    /// X и Y — план (Y вниз), Z — высота, мм.
    pub p: DVec3,
    /// Единичная нормаль (знак может быть любым).
    pub n: DVec3,
    /// Площадь поверхности, которую представляет точка, мм².
    pub w: f64,
}

/// Наибольшее число точек выборки.
const MAX_SAMPLES: f64 = 2_500_000.0;
/// Сторона ячейки выборки по сетке, мм.
const CELL: f64 = 35.0;
/// Сторона вокселя для оценки нормалей облака, мм.
const VOXEL: f64 = 80.0;

/// Ось «вверх» в координатах файла: номер оси и знак.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Axis {
    index: usize,
    positive: bool,
}

/// Переводит точку файла (уже в мм) в план: две оставшиеся оси по циклу
/// после «вверх» — X и −Y плана (правая система сохраняется).
fn to_plan(v: DVec3, up: Axis) -> DVec3 {
    let c = v.to_array();
    let (u, a, b) = (c[up.index], c[(up.index + 1) % 3], c[(up.index + 2) % 3]);
    if up.positive {
        DVec3::new(a, -b, u)
    } else {
        DVec3::new(a, b, -u)
    }
}

/// Масштаб в мм по размеру сцены: квартира — единицы–десятки метров.
fn guess_mm(points: &[DVec3]) -> f64 {
    let (lo, hi) = points.iter().fold(
        (DVec3::splat(f64::MAX), DVec3::splat(f64::MIN)),
        |(lo, hi), p| (lo.min(*p), hi.max(*p)),
    );
    let diagonal = (hi - lo).length();
    if diagonal < 150.0 {
        1000.0
    } else if diagonal < 5000.0 {
        10.0
    } else {
        1.0
    }
}

fn triangle_normals(points: &[DVec3], triangles: &[[u32; 3]]) -> Vec<(DVec3, f64)> {
    triangles
        .iter()
        .map(|t| {
            let [a, b, c] = t.map(|i| points[i as usize]);
            let cross = (b - a).cross(c - a);
            let len = cross.length();
            (if len > 0.0 { cross / len } else { DVec3::ZERO }, len / 2.0)
        })
        .collect()
}

/// Наименьший собственный вектор симметричной матрицы 3×3 (метод Якоби).
#[allow(clippy::needless_range_loop)] // индексы строк и столбцов матрицы
fn smallest_eigenvector(m: DMat3) -> DVec3 {
    let mut a = m.to_cols_array_2d();
    let mut v = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    for _ in 0..24 {
        let (mut p, mut q, mut max) = (0, 1, 0.0);
        for i in 0..3 {
            for j in (i + 1)..3 {
                if a[i][j].abs() > max {
                    max = a[i][j].abs();
                    p = i;
                    q = j;
                }
            }
        }
        if max < 1e-12 {
            break;
        }
        let theta = (a[q][q] - a[p][p]) / (2.0 * a[p][q]);
        let t = theta.signum() / (theta.abs() + (theta * theta + 1.0).sqrt());
        let c = 1.0 / (t * t + 1.0).sqrt();
        let s = t * c;
        for k in 0..3 {
            let (akp, akq) = (a[k][p], a[k][q]);
            a[k][p] = c * akp - s * akq;
            a[k][q] = s * akp + c * akq;
        }
        for k in 0..3 {
            let (apk, aqk) = (a[p][k], a[q][k]);
            a[p][k] = c * apk - s * aqk;
            a[q][k] = s * apk + c * aqk;
        }
        for row in &mut v {
            let (vp, vq) = (row[p], row[q]);
            row[p] = c * vp - s * vq;
            row[q] = s * vp + c * vq;
        }
    }
    let i = (0..3)
        .min_by(|&x, &y| a[x][x].total_cmp(&a[y][y]))
        .unwrap_or(0);
    DVec3::new(v[0][i], v[1][i], v[2][i]).normalize_or(DVec3::Z)
}

fn voxel(p: DVec3) -> (i64, i64, i64) {
    #[allow(clippy::cast_possible_truncation)] // координаты квартиры в мм далеко от пределов i64
    let k = |v: f64| (v / VOXEL).floor() as i64;
    (k(p.x), k(p.y), k(p.z))
}

/// Нормали и веса точек облака по соседям в вокселе.
fn cloud_normals(points: &[DVec3]) -> Vec<(DVec3, f64)> {
    let mut cells: HashMap<(i64, i64, i64), Vec<usize>> = HashMap::new();
    for (i, p) in points.iter().enumerate() {
        cells.entry(voxel(*p)).or_default().push(i);
    }
    let mut out = vec![(DVec3::ZERO, 0.0); points.len()];
    for list in cells.values() {
        #[allow(clippy::cast_precision_loss)] // число точек в вокселе мало
        let count = list.len() as f64;
        let weight = VOXEL * VOXEL / count;
        if list.len() < 5 {
            continue;
        }
        let mean = list.iter().map(|&i| points[i]).sum::<DVec3>() / count;
        let mut cov = DMat3::ZERO;
        for &i in list {
            let d = points[i] - mean;
            cov += DMat3::from_cols(d * d.x, d * d.y, d * d.z);
        }
        let n = smallest_eigenvector(cov / count);
        for &i in list {
            out[i] = (n, weight);
        }
    }
    out
}

/// Определяет ось «вверх»: по ней больше всего горизонтальных поверхностей
/// (пол и потолок); знак — в нижней половине больше горизонтального (пол и мебель).
fn detect_up(points: &[DVec3], normals: &[(DVec3, f64)]) -> Axis {
    let mut weights = [0.0f64; 3];
    for (n, w) in normals {
        for (k, weight) in weights.iter_mut().enumerate() {
            if n[k].abs() > 0.9 {
                *weight += w;
            }
        }
    }
    let index = (0..3)
        .max_by(|&a, &b| weights[a].total_cmp(&weights[b]))
        .unwrap_or(1);
    let (lo, hi) = points.iter().fold((f64::MAX, f64::MIN), |(lo, hi), p| {
        (lo.min(p[index]), hi.max(p[index]))
    });
    let mid = f64::midpoint(lo, hi);
    let (mut below, mut above) = (0.0, 0.0);
    for (p, (n, w)) in points.iter().zip(normals) {
        if n[index].abs() > 0.9 {
            if p[index] < mid {
                below += w;
            } else {
                above += w;
            }
        }
    }
    Axis {
        index,
        positive: below >= above,
    }
}

/// Строит выборку поверхности скана.
#[must_use]
pub fn build(loaded: &Loaded) -> Vec<Sample> {
    let mesh = &loaded.mesh;
    let cloud = &loaded.cloud;
    let raw: &[DVec3] = if mesh.triangles.is_empty() {
        &cloud.points
    } else {
        &mesh.positions
    };
    if raw.is_empty() {
        return Vec::new();
    }
    let scale = loaded.to_mm.unwrap_or_else(|| guess_mm(raw));
    let points: Vec<DVec3> = raw.iter().map(|p| *p * scale).collect();

    if mesh.triangles.is_empty() {
        let normals = match &cloud.normals {
            Some(n) if n.iter().any(|v| v.length_squared() > 0.5) => {
                let base = cloud_normals(&points);
                n.iter()
                    .zip(base)
                    .map(|(n, (_, w))| (n.normalize_or_zero(), w))
                    .collect()
            }
            _ => cloud_normals(&points),
        };
        let up = match loaded.up {
            Some(Up::Y) => Axis {
                index: 1,
                positive: true,
            },
            _ => detect_up(&points, &normals),
        };
        #[allow(clippy::cast_precision_loss)] // число точек < 2^52
        let stride = (points.len() as f64 / MAX_SAMPLES).ceil().max(1.0);
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // шаг прореживания ≥ 1
        let stride = stride as usize;
        #[allow(clippy::cast_precision_loss)] // шаг мал
        let boost = stride as f64;
        return points
            .iter()
            .zip(&normals)
            .step_by(stride)
            .filter(|(_, (n, w))| *w > 0.0 && n.length_squared() > 0.5)
            .map(|(p, (n, w))| Sample {
                p: to_plan(*p, up),
                n: to_plan(*n, up).normalize_or_zero(),
                w: w * boost,
            })
            .collect();
    }

    let normals = triangle_normals(&points, &mesh.triangles);
    let up = match loaded.up {
        Some(Up::Y) => Axis {
            index: 1,
            positive: true,
        },
        _ => {
            let centers: Vec<DVec3> = mesh
                .triangles
                .iter()
                .map(|t| t.iter().map(|&i| points[i as usize]).sum::<DVec3>() / 3.0)
                .collect();
            detect_up(&centers, &normals)
        }
    };
    let total: f64 = normals.iter().map(|(_, a)| a).sum();
    let cell = CELL.max((total / MAX_SAMPLES).sqrt());
    let mut out = Vec::new();
    // Равномерные точки треугольника: последовательность R2 (иррациональные шаги).
    const G1: f64 = 0.754_877_666_246_692_8;
    const G2: f64 = 0.569_840_290_998_053_3;
    for (t, (n, area)) in mesh.triangles.iter().zip(&normals) {
        if *area <= 0.0 {
            continue;
        }
        let [a, b, c] = t.map(|i| points[i as usize]);
        let k = (area / (cell * cell)).ceil().clamp(1.0, 4096.0);
        let w = area / k;
        let normal = to_plan(*n, up).normalize_or_zero();
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // k ∈ [1, 4096]
        let count = k as u32;
        for i in 0..count {
            let i = f64::from(i);
            let (mut u, mut v) = ((0.5 + G1 * i).fract(), (0.5 + G2 * i).fract());
            if u + v > 1.0 {
                (u, v) = (1.0 - u, 1.0 - v);
            }
            let p = a + (b - a) * u + (c - a) * v;
            out.push(Sample {
                p: to_plan(p, up),
                n: normal,
                w,
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn y_up_maps_to_plan() {
        let up = Axis {
            index: 1,
            positive: true,
        };
        // Ось Y файла — высота; X и Z — план.
        assert_eq!(
            to_plan(DVec3::new(1.0, 2.0, 3.0), up),
            DVec3::new(3.0, -1.0, 2.0)
        );
    }

    #[test]
    fn eigenvector_of_flat_cloud_is_normal() {
        let m = DMat3::from_diagonal(DVec3::new(4.0, 1e-4, 9.0));
        let n = smallest_eigenvector(m);
        assert!((n.y.abs() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn units_are_guessed() {
        assert!((guess_mm(&[DVec3::ZERO, DVec3::new(5.0, 3.0, 2.7)]) - 1000.0).abs() < 1e-9);
        assert!((guess_mm(&[DVec3::ZERO, DVec3::new(5000.0, 3000.0, 2700.0)]) - 1.0).abs() < 1e-9);
    }
}
