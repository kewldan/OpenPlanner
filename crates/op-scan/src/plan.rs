//! План из результата: ответы на вопросы, стыки стен, узлы.

use glam::DVec2;
use op_model::{Justification, OpeningKind};

use crate::{
    IssueKind, PlanOpening, PlanWall, Point, ScanAnswer, ScanIssue, ScanParams, ScanPlan,
    ScanResult, ScanWall, WallSource,
};

/// Стена плана до стыков.
#[derive(Debug, Clone)]
struct Seg {
    id: Option<u32>,
    a: DVec2,
    b: DVec2,
    thickness: f64,
    justification: Justification,
}

impl Seg {
    fn dir(&self) -> DVec2 {
        (self.b - self.a).normalize_or_zero()
    }

    fn len(&self) -> f64 {
        self.a.distance(self.b)
    }
}

/// Принят ли вопрос: ответ пользователя или по умолчанию — уверенность от 0,5.
#[must_use]
pub fn accepted(issue: &ScanIssue, answers: &[ScanAnswer]) -> bool {
    answers
        .iter()
        .find(|a| a.issue == issue.id)
        .map_or(issue.confidence >= 0.5, |a| a.accept)
}

/// Продлевает и обрезает концы стен до пересечений с соседними (углы и Т-стыки).
fn snap(segs: &mut [Seg]) {
    for _ in 0..2 {
        for i in 0..segs.len() {
            for end in 0..2 {
                let (p, di) = {
                    let s = &segs[i];
                    (if end == 0 { s.a } else { s.b }, s.dir())
                };
                let mut best: Option<(f64, DVec2)> = None;
                for (j, other) in segs.iter().enumerate() {
                    if i == j {
                        continue;
                    }
                    let dj = other.dir();
                    let denom = di.perp_dot(dj);
                    if denom.abs() < 20f64.to_radians().sin() {
                        continue;
                    }
                    let t = (other.a - p).perp_dot(dj) / denom;
                    let x = p + di * t;
                    let tol = 350.0 + segs[i].thickness.max(other.thickness);
                    let along = (x - other.a).dot(dj);
                    if x.distance(p) <= tol
                        && along >= -tol
                        && along <= other.len() + tol
                        && best.is_none_or(|(d, _)| x.distance(p) < d)
                    {
                        best = Some((x.distance(p), x));
                    }
                }
                if let Some((_, x)) = best {
                    if end == 0 {
                        segs[i].a = x;
                    } else {
                        segs[i].b = x;
                    }
                }
            }
        }
    }
}

/// Узлы: концы ближе 20 мм сливаются.
fn nodes(segs: &[Seg]) -> (Vec<DVec2>, Vec<(usize, usize)>) {
    let mut nodes: Vec<DVec2> = Vec::new();
    let mut find = |p: DVec2| {
        if let Some(i) = nodes.iter().position(|n| n.distance(p) <= 20.0) {
            i
        } else {
            nodes.push(p);
            nodes.len() - 1
        }
    };
    let ends = segs.iter().map(|s| (find(s.a), find(s.b))).collect();
    (nodes, ends)
}

fn segs_of(walls: &[ScanWall], single: f64) -> Vec<Seg> {
    walls
        .iter()
        .map(|w| Seg {
            id: Some(w.id),
            a: w.a.vec(),
            b: w.b.vec(),
            thickness: if w.source == WallSource::Single {
                single
            } else {
                w.thickness
            },
            justification: w.justification,
        })
        .collect()
}

/// Концы стен, не примыкающие ни к чему, — для вопросов «пропущена стена».
fn dangling(segs: &[Seg]) -> Vec<DVec2> {
    let (nodes, ends) = nodes(segs);
    let mut degree = vec![0; nodes.len()];
    for (a, b) in &ends {
        degree[*a] += 1;
        degree[*b] += 1;
    }
    nodes
        .iter()
        .enumerate()
        .filter(|(i, p)| {
            degree[*i] == 1
                && !segs.iter().any(|s| {
                    let t = (**p - s.a).dot(s.dir());
                    t > 1.0 && t < s.len() - 1.0 && (s.a + s.dir() * t).distance(**p) <= 5.0
                })
        })
        .map(|(_, p)| *p)
        .collect()
}

/// Вопросы «пропущена стена»: пары свободных концов до 4 м друг от друга,
/// соединение которых идёт вдоль одной из ориентаций стен.
#[must_use]
pub fn missing_walls(result: &ScanResult, first_id: u32) -> Vec<ScanIssue> {
    let kept: Vec<ScanWall> = result
        .walls
        .iter()
        .filter(|w| {
            !result.issues.iter().any(|i| {
                matches!(i.kind, IssueKind::ExtraWall { wall } if wall == w.id) && accepted(i, &[])
            })
        })
        .cloned()
        .collect();
    let mut segs = segs_of(&kept, crate::assemble::SINGLE_THICKNESS);
    snap(&mut segs);
    let mut free = dangling(&segs);
    let directions: Vec<DVec2> = segs.iter().map(Seg::dir).collect();
    let mut issues = Vec::new();
    let mut id = first_id;
    while let Some(p) = free.pop() {
        let candidate = free
            .iter()
            .enumerate()
            .filter(|(_, q)| {
                let d = **q - p;
                let len = d.length();
                len > 100.0
                    && len <= 4000.0
                    && directions
                        .iter()
                        .any(|dir| (d / len).perp_dot(*dir).abs() <= 10f64.to_radians().sin())
            })
            .min_by(|(_, x), (_, y)| x.distance(p).total_cmp(&y.distance(p)));
        if let Some((k, q)) = candidate {
            let q = *q;
            free.remove(k);
            issues.push(ScanIssue {
                id,
                kind: IssueKind::MissingWall {
                    a: Point::from_vec(p),
                    b: Point::from_vec(q),
                },
                confidence: (0.9 - p.distance(q) / 8000.0).clamp(0.4, 0.9),
                at: Point::from_vec(p.midpoint(q)),
            });
            id += 1;
        }
    }
    issues
}

/// Строит план: применяет ответы и параметры, сводит стыки и узлы.
#[must_use]
pub fn apply(result: &ScanResult, answers: &[ScanAnswer], params: &ScanParams) -> ScanPlan {
    let removed: Vec<u32> = result
        .issues
        .iter()
        .filter_map(|i| match i.kind {
            IssueKind::ExtraWall { wall } if accepted(i, answers) => Some(wall),
            _ => None,
        })
        .collect();
    let kept: Vec<ScanWall> = result
        .walls
        .iter()
        .filter(|w| !removed.contains(&w.id))
        .cloned()
        .collect();
    let mut segs = segs_of(&kept, params.single_thickness);
    for issue in &result.issues {
        if let IssueKind::MissingWall { a, b } = issue.kind
            && accepted(issue, answers)
        {
            segs.push(Seg {
                id: None,
                a: a.vec(),
                b: b.vec(),
                thickness: params.added_thickness,
                justification: Justification::Center,
            });
        }
    }
    snap(&mut segs);
    segs.retain(|s| s.len() >= 100.0);
    let (nodes, ends) = nodes(&segs);
    let walls: Vec<PlanWall> = segs
        .iter()
        .zip(&ends)
        .filter(|(_, (a, b))| a != b)
        .map(|(s, (a, b))| PlanWall {
            a: *a,
            b: *b,
            thickness: s.thickness,
            justification: s.justification,
        })
        .collect();
    let index: Vec<Option<u32>> = segs
        .iter()
        .zip(&ends)
        .filter(|(_, (a, b))| a != b)
        .map(|(s, _)| s.id)
        .collect();
    let openings = result
        .openings
        .iter()
        .filter_map(|o| {
            let w = index.iter().position(|id| *id == Some(o.wall))?;
            let wall = &walls[w];
            let (a, b) = (nodes[wall.a], nodes[wall.b]);
            let len = a.distance(b);
            let offset = (o.center.vec() - a).dot((b - a).normalize_or_zero());
            if len < o.width {
                return None;
            }
            let kind = result
                .issues
                .iter()
                .find(|i| matches!(i.kind, IssueKind::DoorOrArch { opening } if opening == o.id))
                .map_or(o.kind, |i| {
                    if accepted(i, answers) {
                        OpeningKind::Door
                    } else {
                        OpeningKind::Arch
                    }
                });
            Some(PlanOpening {
                wall: w,
                kind,
                offset: offset.clamp(o.width / 2.0, len - o.width / 2.0),
                width: o.width,
                height: o.height,
                sill: o.sill,
            })
        })
        .collect();
    ScanPlan {
        nodes,
        walls,
        openings,
    }
}
