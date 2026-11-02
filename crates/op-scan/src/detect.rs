//! Распознавание по выборке поверхности: пол и потолок, ориентации стен,
//! грани стен, их сегменты и проёмы.

use glam::{DVec2, DVec3};

use crate::samples::Sample;

/// Высота пола и потолка, мм.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Levels {
    /// Пол.
    pub floor: f64,
    /// Потолок, если найден.
    pub ceiling: Option<f64>,
}

impl Levels {
    /// Потолок или пол + высота по умолчанию.
    #[must_use]
    pub fn top(&self) -> f64 {
        self.ceiling.unwrap_or(self.floor + DEFAULT_HEIGHT)
    }
}

/// Высота потолка, если он не найден, мм.
pub const DEFAULT_HEIGHT: f64 = 2700.0;
/// Наименьшая площадь пола, мм².
const MIN_FLOOR: f64 = 1.0e6;
/// Наименьшая площадь грани стены в полосе поиска, мм².
const MIN_FACE: f64 = 2.5e5;
/// Шаг сетки грани вдоль стены, мм.
const COL: f64 = 50.0;
/// Шаг сетки грани по высоте, мм.
const ROW: f64 = 100.0;

fn is_horizontal(s: &Sample) -> bool {
    s.n.z.abs() > 0.9
}

fn is_vertical(s: &Sample) -> bool {
    s.n.z.abs() < 0.3
}

/// Гистограмма с шагом `step` от `lo`.
fn histogram(values: impl Iterator<Item = (f64, f64)>, lo: f64, hi: f64, step: f64) -> Vec<f64> {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // диапазон сцены ограничен
    let n = (((hi - lo) / step).ceil() as usize).clamp(1, 200_000);
    let mut bins = vec![0.0; n];
    for (v, w) in values {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // внутри диапазона
        let i = (((v - lo) / step).floor().max(0.0) as usize).min(n - 1);
        bins[i] += w;
    }
    bins
}

fn smooth(bins: &[f64], radius: usize, circular: bool) -> Vec<f64> {
    let n = bins.len();
    (0..n)
        .map(|i| {
            let mut sum = 0.0;
            for d in 0..=2 * radius {
                let k = i + d;
                if circular {
                    sum += bins[(k + n - radius % n) % n];
                } else if k >= radius && k - radius < n {
                    sum += bins[k - radius];
                }
            }
            sum
        })
        .collect()
}

/// Пол — самый весомый горизонтальный уровень в нижней части сцены;
/// потолок — самый весомый на 1,9–5 м выше.
#[must_use]
pub fn levels(samples: &[Sample]) -> Option<Levels> {
    let horizontal: Vec<&Sample> = samples.iter().filter(|s| is_horizontal(s)).collect();
    let (lo, hi) = horizontal.iter().fold((f64::MAX, f64::MIN), |(lo, hi), s| {
        (lo.min(s.p.z), hi.max(s.p.z))
    });
    if horizontal.is_empty() || hi < lo {
        return None;
    }
    let step = 20.0;
    let bins = smooth(
        &histogram(horizontal.iter().map(|s| (s.p.z, s.w)), lo, hi + step, step),
        1,
        false,
    );
    let at = |i: usize| lo + (f64::from(u32::try_from(i).unwrap_or(0)) + 0.5) * step;
    let lower = lo + (hi - lo).max(0.0) * 0.45;
    let (fi, fw) = bins
        .iter()
        .enumerate()
        .filter(|(i, _)| at(*i) <= lower.max(lo + step))
        .max_by(|a, b| a.1.total_cmp(b.1))?;
    if *fw < MIN_FLOOR {
        return None;
    }
    let refine = |center: f64| {
        let (sum, weight) = horizontal
            .iter()
            .filter(|s| (s.p.z - center).abs() <= 40.0)
            .fold((0.0, 0.0), |(s, w), x| (s + x.p.z * x.w, w + x.w));
        if weight > 0.0 { sum / weight } else { center }
    };
    let floor = refine(at(fi));
    let ceiling = bins
        .iter()
        .enumerate()
        .filter(|(i, _)| (floor + 1900.0..=floor + 5000.0).contains(&at(*i)))
        .max_by(|a, b| a.1.total_cmp(b.1))
        .filter(|(_, w)| **w >= 0.15 * fw)
        .map(|(i, _)| refine(at(i)));
    Some(Levels { floor, ceiling })
}

/// Полоса высот для поиска стен: выше мебели, ниже потолка.
fn band(levels: &Levels) -> (f64, f64) {
    (levels.floor + 1000.0, levels.top() - 200.0)
}

/// Угол линии нормали в [0, π).
fn line_angle(n: DVec2) -> f64 {
    n.y.atan2(n.x).rem_euclid(std::f64::consts::PI)
}

fn angle_diff(a: f64, b: f64) -> f64 {
    let d = (a - b).rem_euclid(std::f64::consts::PI);
    d.min(std::f64::consts::PI - d)
}

/// Ориентации стен (углы нормалей, рад, в [0, π)), по убыванию веса.
#[must_use]
pub fn orientations(samples: &[Sample], levels: &Levels) -> Vec<f64> {
    let (z0, z1) = band(levels);
    let vertical: Vec<(f64, f64)> = samples
        .iter()
        .filter(|s| is_vertical(s) && (z0..=z1).contains(&s.p.z))
        .filter_map(|s| {
            let n = s.n.truncate();
            (n.length() > 0.5).then(|| (line_angle(n), s.w))
        })
        .collect();
    let bins = smooth(
        &histogram(
            vertical.iter().copied(),
            0.0,
            std::f64::consts::PI,
            std::f64::consts::PI / 180.0,
        ),
        2,
        true,
    );
    let max = bins.iter().copied().fold(0.0, f64::max);
    if max <= 0.0 {
        return Vec::new();
    }
    let n = bins.len();
    let mut peaks: Vec<(usize, f64)> = (0..n)
        .filter(|&i| {
            bins[i] >= 0.12 * max
                && bins[i] >= bins[(i + n - 1) % n]
                && bins[i] >= bins[(i + 1) % n]
        })
        .map(|i| (i, bins[i]))
        .collect();
    peaks.sort_by(|a, b| b.1.total_cmp(&a.1));
    let mut chosen: Vec<f64> = Vec::new();
    for (i, _) in peaks {
        let a = (f64::from(u32::try_from(i).unwrap_or(0)) + 0.5).to_radians();
        if chosen
            .iter()
            .all(|c| angle_diff(*c, a) > 15f64.to_radians())
        {
            chosen.push(a);
        }
    }
    // Уточнение: круговое среднее удвоенных углов в окрестности ±5°.
    chosen
        .into_iter()
        .map(|a| {
            let (c, s) = vertical
                .iter()
                .filter(|(v, _)| angle_diff(*v, a) <= 5f64.to_radians())
                .fold((0.0, 0.0), |(c, s), (v, w)| {
                    (c + w * (2.0 * v).cos(), s + w * (2.0 * v).sin())
                });
            if c == 0.0 && s == 0.0 {
                a
            } else {
                (s.atan2(c) / 2.0).rem_euclid(std::f64::consts::PI)
            }
        })
        .collect()
}

/// Проём, найденный на грани: пустота в сетке заполненности.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FaceOpening {
    /// Начало вдоль грани, мм.
    pub t0: f64,
    /// Конец вдоль грани, мм.
    pub t1: f64,
    /// Низ над полом, мм.
    pub bottom: f64,
    /// Верх над полом, мм.
    pub top: f64,
    /// Начинается от пола (дверь или арка), иначе — окно.
    pub from_floor: bool,
}

/// Непрерывный участок грани.
#[derive(Debug, Clone, PartialEq)]
pub struct Segment {
    /// Начало вдоль грани, мм.
    pub t0: f64,
    /// Конец вдоль грани, мм.
    pub t1: f64,
    /// Доля колонок, которые доходят до потолка.
    pub top_reach: f64,
    /// Средняя заполненность колонок.
    pub support: f64,
    /// Проёмы.
    pub openings: Vec<FaceOpening>,
}

/// Грань стены: плоскость `p · n = offset`.
#[derive(Debug, Clone, PartialEq)]
pub struct Face {
    /// Номер ориентации.
    pub orientation: usize,
    /// Нормаль ориентации (единичная, на плане).
    pub normal: DVec2,
    /// Смещение плоскости, мм.
    pub offset: f64,
    /// С какой стороны помещение: +1 — по нормали, −1 — против.
    pub interior: f64,
    /// Участки.
    pub segments: Vec<Segment>,
}

impl Face {
    /// Направление вдоль грани (координата `t`).
    #[must_use]
    pub fn dir(&self) -> DVec2 {
        DVec2::new(-self.normal.y, self.normal.x)
    }

    /// Точка плоскости грани на координате `t`.
    #[must_use]
    pub fn point(&self, t: f64) -> DVec2 {
        self.normal * self.offset + self.dir() * t
    }
}

/// Грани всех ориентаций.
#[must_use]
pub fn faces(samples: &[Sample], levels: &Levels, angles: &[f64]) -> Vec<Face> {
    let floor_samples: Vec<DVec2> = samples
        .iter()
        .filter(|s| is_horizontal(s) && (s.p.z - levels.floor).abs() < 60.0)
        .map(|s| s.p.truncate())
        .collect();
    let mut out = Vec::new();
    for (k, &angle) in angles.iter().enumerate() {
        let n = DVec2::new(angle.cos(), angle.sin());
        let oriented: Vec<&Sample> = samples
            .iter()
            .filter(|s| is_vertical(s))
            .filter(|s| {
                let m = s.n.truncate();
                m.length() > 0.5 && angle_diff(line_angle(m), angle) <= 7f64.to_radians()
            })
            .filter(|s| s.p.z > levels.floor + 40.0 && s.p.z < levels.top() - 40.0)
            .collect();
        out.extend(faces_of(&oriented, &floor_samples, levels, k, n));
    }
    out
}

fn faces_of(
    oriented: &[&Sample],
    floor: &[DVec2],
    levels: &Levels,
    k: usize,
    n: DVec2,
) -> Vec<Face> {
    let (z0, z1) = band(levels);
    let in_band: Vec<(f64, f64)> = oriented
        .iter()
        .filter(|s| (z0..=z1).contains(&s.p.z))
        .map(|s| (s.p.truncate().dot(n), s.w))
        .collect();
    let (lo, hi) = in_band
        .iter()
        .fold((f64::MAX, f64::MIN), |(lo, hi), (d, _)| {
            (lo.min(*d), hi.max(*d))
        });
    if in_band.is_empty() {
        return Vec::new();
    }
    let step = 10.0;
    let bins = smooth(
        &histogram(in_band.iter().copied(), lo - step, hi + step, step),
        2,
        false,
    );
    let at = |i: usize| lo - step + (f64::from(u32::try_from(i).unwrap_or(0)) + 0.5) * step;
    let mut peaks: Vec<(f64, f64)> = (0..bins.len())
        .filter(|&i| {
            bins[i] >= MIN_FACE
                && (i == 0 || bins[i] >= bins[i - 1])
                && (i + 1 == bins.len() || bins[i] > bins[i + 1])
        })
        .map(|i| (at(i), bins[i]))
        .collect();
    peaks.sort_by(|a, b| b.1.total_cmp(&a.1));
    let mut offsets: Vec<f64> = Vec::new();
    for (d, _) in peaks {
        if offsets.iter().all(|o| (o - d).abs() > 60.0) {
            offsets.push(d);
        }
    }
    offsets
        .into_iter()
        .filter_map(|d| {
            let (sum, weight) = in_band
                .iter()
                .filter(|(v, _)| (v - d).abs() <= 25.0)
                .fold((0.0, 0.0), |(s, w), (v, x)| (s + v * x, w + x));
            let offset = if weight > 0.0 { sum / weight } else { d };
            face(oriented, floor, levels, k, n, offset)
        })
        .collect()
}

fn face(
    oriented: &[&Sample],
    floor: &[DVec2],
    levels: &Levels,
    k: usize,
    n: DVec2,
    offset: f64,
) -> Option<Face> {
    let dir = DVec2::new(-n.y, n.x);
    let members: Vec<&Sample> = oriented
        .iter()
        .copied()
        .filter(|s| (s.p.truncate().dot(n) - offset).abs() <= 35.0)
        .collect();
    let (t_lo, t_hi) = members.iter().fold((f64::MAX, f64::MIN), |(lo, hi), s| {
        let t = s.p.truncate().dot(dir);
        (lo.min(t), hi.max(t))
    });
    if members.is_empty() || t_hi - t_lo < 300.0 {
        return None;
    }
    // Сторона помещения: пол вплотную к грани.
    let near = |side: f64| {
        floor
            .iter()
            .filter(|p| {
                let d = side * (p.dot(n) - offset);
                let t = p.dot(dir);
                (10.0..=75.0).contains(&d) && t >= t_lo && t <= t_hi
            })
            .count()
    };
    let (plus, minus) = (near(1.0), near(-1.0));
    let interior = if plus == minus {
        let vote: f64 = members
            .iter()
            .map(|s| s.n.truncate().dot(n).signum() * s.w)
            .sum();
        if vote >= 0.0 { 1.0 } else { -1.0 }
    } else if plus > minus {
        1.0
    } else {
        -1.0
    };
    let segments = segments(
        &members, oriented, levels, n, dir, offset, interior, t_lo, t_hi,
    );
    (!segments.is_empty()).then_some(Face {
        orientation: k,
        normal: n,
        offset,
        interior,
        segments,
    })
}

#[allow(clippy::too_many_arguments)] // параметры одной грани
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)] // индексы сетки
fn segments(
    members: &[&Sample],
    oriented: &[&Sample],
    levels: &Levels,
    n: DVec2,
    dir: DVec2,
    offset: f64,
    interior: f64,
    t_lo: f64,
    t_hi: f64,
) -> Vec<Segment> {
    let height = levels.top() - levels.floor;
    let cols = (((t_hi - t_lo) / COL).ceil() as usize + 1).min(2000);
    let rows = ((height / ROW).ceil() as usize).max(1);
    let mut grid = vec![0.0f64; cols * rows];
    for s in members {
        let c = (((s.p.truncate().dot(dir) - t_lo) / COL).floor().max(0.0) as usize).min(cols - 1);
        let r = (((s.p.z - levels.floor) / ROW).floor().max(0.0) as usize).min(rows - 1);
        grid[r * cols + c] += s.w;
    }
    let fill = |c: usize, r: usize| (grid[r * cols + c] / (COL * ROW)).min(1.0);
    let filled = |c: usize, r: usize| fill(c, r) >= 0.45;
    // Строки без кромок у пола и потолка.
    let r0 = 1.min(rows - 1);
    let r1 = rows.saturating_sub(1).max(r0 + 1);
    let coverage: Vec<f64> = (0..cols)
        .map(|c| (r0..r1).filter(|&r| filled(c, r)).count() as f64 / (r1 - r0) as f64)
        .collect();
    let top_band = ((height - 400.0) / ROW).floor().max(0.0) as usize;
    let reaches_top = |c: usize| (top_band.min(rows - 1)..rows).any(|r| filled(c, r));

    // Участки: колонки с заполненностью, промежутки до 200 мм — шум.
    let present: Vec<bool> = coverage.iter().map(|&v| v >= 0.08).collect();
    let mut runs: Vec<(usize, usize)> = Vec::new();
    let mut c = 0;
    while c < cols {
        if !present[c] {
            c += 1;
            continue;
        }
        let start = c;
        let mut end = c;
        let mut gap = 0;
        c += 1;
        while c < cols {
            if present[c] {
                end = c;
                gap = 0;
            } else {
                gap += 1;
                if gap > 4 {
                    break;
                }
            }
            c += 1;
        }
        runs.push((start, end));
    }

    let t = |c: usize| t_lo + c as f64 * COL;
    runs.into_iter()
        .filter(|(a, b)| (b - a + 1) as f64 * COL >= 300.0)
        .map(|(a, b)| {
            let count = (b - a + 1) as f64;
            let openings = openings(rows, a, b, &filled)
                .into_iter()
                .map(|(c0, c1, bottom, top, from_floor)| {
                    // Края уточняются по доле пустоты соседних колонок и строк.
                    let empty_col = |c: usize| {
                        (bottom..top).map(|r| 1.0 - fill(c, r)).sum::<f64>()
                            / (top - bottom).max(1) as f64
                    };
                    let empty_row = |r: usize| {
                        (c0..=c1).map(|c| 1.0 - fill(c, r)).sum::<f64>() / (c1 - c0 + 1) as f64
                    };
                    let left = if c0 > 0 { empty_col(c0 - 1) } else { 0.0 };
                    let right = if c1 + 1 < cols {
                        empty_col(c1 + 1)
                    } else {
                        0.0
                    };
                    let below = if bottom > 0 && !from_floor {
                        empty_row(bottom - 1)
                    } else {
                        0.0
                    };
                    let above = if top < rows { empty_row(top) } else { 0.0 };
                    FaceOpening {
                        t0: t(c0) - left * COL,
                        t1: t(c1 + 1) + right * COL,
                        bottom: if from_floor {
                            0.0
                        } else {
                            bottom as f64 * ROW - below * ROW
                        },
                        top: top as f64 * ROW + above * ROW,
                        from_floor,
                    }
                })
                .filter(|o| !occluded(o, oriented, levels, n, dir, offset, interior))
                .collect();
            Segment {
                t0: t(a),
                t1: t(b + 1),
                top_reach: (a..=b).filter(|&c| reaches_top(c)).count() as f64 / count,
                support: (a..=b).map(|c| coverage[c]).sum::<f64>() / count,
                openings,
            }
        })
        .collect()
}

/// Пустоты в колонках участка, сгруппированные в проёмы:
/// (первая колонка, последняя, нижняя строка, верхняя строка, от пола).
#[allow(clippy::cast_precision_loss)] // номера колонок и строк сетки малы
fn openings(
    rows: usize,
    a: usize,
    b: usize,
    filled: &dyn Fn(usize, usize) -> bool,
) -> Vec<(usize, usize, usize, usize, bool)> {
    // Классификация колонки: самая высокая пустота с заполнением над ней.
    let classify = |c: usize| -> Option<(usize, usize, bool)> {
        let top_limit = rows.saturating_sub(1);
        let mut best: Option<(usize, usize)> = None;
        let mut r = 0;
        while r < top_limit {
            if filled(c, r) {
                r += 1;
                continue;
            }
            let start = r;
            while r < top_limit && !filled(c, r) {
                r += 1;
            }
            // Пустота [start, r) должна быть закрыта сверху (перемычка).
            if r < rows && filled(c, r) && best.is_none_or(|(s, e)| r - start > e - s) {
                best = Some((start, r));
            }
        }
        let (bottom, top) = best?;
        let from_floor = bottom <= 2;
        let tall = (top - bottom) as f64 * ROW;
        if from_floor && top as f64 * ROW >= 1700.0 {
            Some((0, top, true))
        } else if !from_floor && bottom as f64 * ROW >= 300.0 && tall >= 400.0 {
            Some((bottom, top, false))
        } else {
            None
        }
    };
    let mut out = Vec::new();
    let mut c = a;
    while c <= b {
        let Some((bottom, top, from_floor)) = classify(c) else {
            c += 1;
            continue;
        };
        let start = c;
        let mut end = c;
        let mut bottoms = vec![bottom];
        let mut tops = vec![top];
        let mut miss = 0;
        c += 1;
        while c <= b {
            match classify(c) {
                Some((b2, t2, f2))
                    if f2 == from_floor && b2.abs_diff(bottom) <= 2 && t2.abs_diff(top) <= 2 =>
                {
                    end = c;
                    bottoms.push(b2);
                    tops.push(t2);
                    miss = 0;
                }
                _ => {
                    miss += 1;
                    if miss > 1 {
                        break;
                    }
                }
            }
            c += 1;
        }
        let width = (end - start + 1) as f64 * COL;
        let min = if from_floor { 550.0 } else { 450.0 };
        // Проём, упёртый в край участка, — это конец стены, а не проём.
        if width >= min && start > a && end < b {
            bottoms.sort_unstable();
            tops.sort_unstable();
            out.push((
                start,
                end,
                bottoms[bottoms.len() / 2],
                tops[tops.len() / 2],
                from_floor,
            ));
        }
    }
    out
}

/// Пустота за мебелью: перед гранью (со стороны помещения) стоит
/// параллельная поверхность, закрывающая её от сканера.
#[allow(clippy::too_many_arguments)] // параметры одной грани
fn occluded(
    o: &FaceOpening,
    oriented: &[&Sample],
    levels: &Levels,
    n: DVec2,
    dir: DVec2,
    offset: f64,
    interior: f64,
) -> bool {
    let area = (o.t1 - o.t0) * (o.top - o.bottom);
    let blocking: f64 = oriented
        .iter()
        .filter(|s| {
            let p = s.p.truncate();
            let d = interior * (p.dot(n) - offset);
            let t = p.dot(dir);
            let h = s.p.z - levels.floor;
            (80.0..=1200.0).contains(&d) && t >= o.t0 && t <= o.t1 && h >= o.bottom && h <= o.top
        })
        .map(|s| s.w)
        .sum();
    blocking >= 0.25 * area
}

/// Поворачивает выборку на угол `angle` вокруг начала координат.
pub fn rotate(samples: &mut [Sample], angle: f64) {
    let (s, c) = angle.sin_cos();
    let r = |v: DVec3| DVec3::new(v.x * c - v.y * s, v.x * s + v.y * c, v.z);
    for sample in samples {
        sample.p = r(sample.p);
        sample.n = r(sample.n);
    }
}
