//! Стены и проёмы из граней, вопросы проверки.

use glam::DVec2;
use op_model::{Justification, OpeningKind};

use crate::{
    IssueKind, Point, ScanIssue, ScanOpening, ScanWall, WallSource,
    detect::{Face, FaceOpening, Levels},
};

/// Толщина стены с одной отсканированной поверхностью по умолчанию, мм.
pub const SINGLE_THICKNESS: f64 = 200.0;

/// Стена до разбора стыков: прямая ориентации `orientation` со смещением
/// `offset`, участок `[t0, t1]` вдоль неё.
#[derive(Debug, Clone)]
struct Line {
    orientation: usize,
    normal: DVec2,
    offset: f64,
    /// 0 — пара граней (по оси), ±1 — сторона помещения у одиночной грани.
    interior: f64,
    thickness: f64,
    t0: f64,
    t1: f64,
    top_reach: f64,
    support: f64,
    openings: Vec<(FaceOpening, f64)>,
}

impl Line {
    fn dir(&self) -> DVec2 {
        DVec2::new(-self.normal.y, self.normal.x)
    }

    fn point(&self, t: f64) -> DVec2 {
        self.normal * self.offset + self.dir() * t
    }

    fn overlap(&self, other: &Self) -> f64 {
        self.t1.min(other.t1) - self.t0.max(other.t0)
    }
}

/// Пары граней: две грани одной ориентации, обращённые друг от друга,
/// на расстоянии толщины стены.
fn lines(faces: &[Face]) -> Vec<Line> {
    let mut used: Vec<Vec<bool>> = faces
        .iter()
        .map(|f| vec![false; f.segments.len()])
        .collect();
    let mut out = Vec::new();
    for (i, a) in faces.iter().enumerate() {
        if a.interior > 0.0 {
            continue;
        }
        for (si, sa) in a.segments.iter().enumerate() {
            // Ближайшая подходящая грань по другую сторону стены.
            let partner = faces
                .iter()
                .enumerate()
                .filter(|(_, b)| b.orientation == a.orientation && b.interior > 0.0)
                .filter(|(_, b)| (60.0..=600.0).contains(&(b.offset - a.offset)))
                .flat_map(|(j, b)| {
                    b.segments
                        .iter()
                        .enumerate()
                        .map(move |(sj, sb)| (j, sj, b, sb))
                })
                .filter(|(j, sj, _, sb)| {
                    !used[*j][*sj] && sa.t1.min(sb.t1) - sa.t0.max(sb.t0) >= 300.0
                })
                .min_by(|x, y| (x.2.offset - a.offset).total_cmp(&(y.2.offset - a.offset)));
            let Some((j, sj, b, sb)) = partner else {
                continue;
            };
            used[i][si] = true;
            used[j][sj] = true;
            let mut openings: Vec<(FaceOpening, f64)> =
                sa.openings.iter().map(|o| (*o, 0.85)).collect();
            for o in &sb.openings {
                // Проём, видимый с обеих сторон, — один проём.
                if !openings
                    .iter()
                    .any(|(p, _)| p.t1.min(o.t1) - p.t0.max(o.t0) > 0.0)
                {
                    openings.push((*o, 0.8));
                }
            }
            out.push(Line {
                orientation: a.orientation,
                normal: a.normal,
                offset: f64::midpoint(a.offset, b.offset),
                interior: 0.0,
                thickness: ((b.offset - a.offset) / 10.0).round() * 10.0,
                t0: sa.t0.min(sb.t0),
                t1: sa.t1.max(sb.t1),
                top_reach: sa.top_reach.max(sb.top_reach),
                support: sa.support.max(sb.support),
                openings,
            });
        }
    }
    for (i, face) in faces.iter().enumerate() {
        for (si, s) in face.segments.iter().enumerate() {
            if used[i][si] {
                continue;
            }
            out.push(Line {
                orientation: face.orientation,
                normal: face.normal,
                offset: face.offset,
                interior: face.interior,
                thickness: SINGLE_THICKNESS,
                t0: s.t0,
                t1: s.t1,
                top_reach: s.top_reach,
                support: s.support,
                openings: s.openings.iter().map(|o| (*o, 0.75)).collect(),
            });
        }
    }
    merge_collinear(out)
}

/// Склеивает участки одной прямой с промежутком до 0,8 м (Т-стык прерывает грань).
fn merge_collinear(mut lines: Vec<Line>) -> Vec<Line> {
    lines.sort_by(|a, b| a.t0.total_cmp(&b.t0));
    let mut out: Vec<Line> = Vec::new();
    for line in lines {
        let target = out.iter_mut().find(|o| {
            o.orientation == line.orientation
                && (o.interior - line.interior).abs() < 0.5
                && (o.offset - line.offset).abs() <= 40.0
                && (o.thickness - line.thickness).abs() <= 40.0
                && line.t0 - o.t1 <= 800.0
                && o.overlap(&line) > -800.0
        });
        match target {
            Some(o) => {
                let (la, lb) = (o.t1 - o.t0, line.t1 - line.t0);
                o.offset = (o.offset * la + line.offset * lb) / (la + lb);
                o.top_reach = (o.top_reach * la + line.top_reach * lb) / (la + lb);
                o.support = (o.support * la + line.support * lb) / (la + lb);
                o.t0 = o.t0.min(line.t0);
                o.t1 = o.t1.max(line.t1);
                o.openings.extend(line.openings);
            }
            None => out.push(line),
        }
    }
    out
}

/// Вероятность того, что проём от пола — дверь (а не арка).
#[must_use]
pub fn door_probability(width: f64, height: f64) -> f64 {
    if width >= 1300.0 || height >= 2350.0 {
        0.2
    } else if width <= 1000.0 && height <= 2200.0 {
        0.8
    } else {
        0.5
    }
}

/// Стены, проёмы и вопросы «лишняя стена?», «дверь или арка?».
#[must_use]
pub fn walls(faces: &[Face], levels: &Levels) -> (Vec<ScanWall>, Vec<ScanOpening>, Vec<ScanIssue>) {
    let mut walls = Vec::new();
    let mut openings = Vec::new();
    let mut issues = Vec::new();
    let mut issue_id = 0;
    for (i, line) in lines(faces).into_iter().enumerate() {
        let id = u32::try_from(i).unwrap_or(u32::MAX);
        // Одиночная грань: тело стены — со стороны, противоположной помещению.
        let (a, b, justification) = if line.interior == 0.0 {
            (
                line.point(line.t0),
                line.point(line.t1),
                Justification::Center,
            )
        } else if line.interior > 0.0 {
            (
                line.point(line.t0),
                line.point(line.t1),
                Justification::Right,
            )
        } else {
            (
                line.point(line.t1),
                line.point(line.t0),
                Justification::Right,
            )
        };
        let length = line.t1 - line.t0;
        let source = if line.interior == 0.0 {
            WallSource::Paired
        } else {
            WallSource::Single
        };
        let confidence = match source {
            WallSource::Paired => 0.9,
            _ => 0.75,
        } * (0.6 + 0.4 * (line.support * 2.0).min(1.0));
        walls.push(ScanWall {
            id,
            a: Point::from_vec(a),
            b: Point::from_vec(b),
            thickness: line.thickness,
            justification,
            source,
            confidence,
        });
        // Не доходит до потолка — мебель (шкаф, кухня); короткая — выступ или шум.
        let extra = if levels.ceiling.is_some() && line.top_reach < 0.5 {
            Some(0.85)
        } else if length < 600.0 {
            Some(0.6)
        } else if line.support < 0.2 {
            Some(0.55)
        } else {
            None
        };
        if let Some(confidence) = extra {
            issues.push(ScanIssue {
                id: issue_id,
                kind: IssueKind::ExtraWall { wall: id },
                confidence,
                at: Point::from_vec(a.midpoint(b)),
            });
            issue_id += 1;
        }
        for (o, confidence) in &line.openings {
            let opening_id = u32::try_from(openings.len()).unwrap_or(u32::MAX);
            let width = ((o.t1 - o.t0) / 10.0).round() * 10.0;
            let center = line.point(f64::midpoint(o.t0, o.t1));
            let (kind, height, sill) = if o.from_floor {
                let p = door_probability(width, o.top);
                if (0.35..=0.65).contains(&p) {
                    issues.push(ScanIssue {
                        id: issue_id,
                        kind: IssueKind::DoorOrArch {
                            opening: opening_id,
                        },
                        confidence: p,
                        at: Point::from_vec(center),
                    });
                    issue_id += 1;
                }
                (
                    if p >= 0.5 {
                        OpeningKind::Door
                    } else {
                        OpeningKind::Arch
                    },
                    o.top,
                    0.0,
                )
            } else {
                (OpeningKind::Window, o.top - o.bottom, o.bottom)
            };
            openings.push(ScanOpening {
                id: opening_id,
                wall: id,
                kind,
                center: Point::from_vec(center),
                width,
                height: (height / 10.0).round() * 10.0,
                sill: (sill / 10.0).round() * 10.0,
                confidence: *confidence,
            });
        }
    }
    (walls, openings, issues)
}
