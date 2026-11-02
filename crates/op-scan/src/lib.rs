//! Распознавание скана: облако точек и LiDAR-сетка → стены, проёмы, помещения.
//!
//! Конвейер ([`analyze`]):
//!
//! 1. чтение файла ([`io`]): GLB, OBJ, PLY, ZIP с ними, параметрическая
//!    комната Apple RoomPlan (JSON);
//! 2. выборка поверхности ([`samples`]): точки плана в мм с нормалями;
//! 3. пол и потолок, ориентации стен, грани и их проёмы ([`detect`]);
//! 4. стены из пар граней и одиночных граней, вопросы проверки ([`assemble`]);
//! 5. вид сверху для подложки ([`raster`]).
//!
//! Ответы пользователя и параметры превращаются в план ([`plan::apply`]):
//! стыки стен сводятся, узлы сливаются. Каждый вывод имеет оценку уверенности.
//! Единицы — миллиметры, ось Y плана направлена вниз.

pub mod assemble;
pub mod detect;
pub mod io;
pub mod plan;
pub mod raster;
pub mod samples;
pub mod synth;
mod types;

use glam::{DVec2, DVec3};
use op_model::{Justification, OpeningKind};

pub use plan::apply;
pub use types::*;

use crate::{
    detect::Levels,
    io::{RoomKind, RoomPlan},
    samples::Sample,
};

/// Ошибка чтения файла скана.
#[derive(Debug, thiserror::Error)]
pub enum ScanError {
    /// Формат не распознан.
    #[error("формат файла не поддерживается: нужен GLB, OBJ, PLY или ZIP с ними")]
    Unsupported,
    /// Файл повреждён.
    #[error("файл повреждён ({0})")]
    Corrupt(&'static str),
    /// В файле нет геометрии.
    #[error("в файле нет геометрии")]
    Empty,
    /// Слишком большой файл.
    #[error("файл слишком большой")]
    TooLarge,
    /// Ошибка чтения.
    #[error("ошибка чтения: {0}")]
    Io(#[from] std::io::Error),
}

/// Этап обработки — для индикатора в мастере импорта.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// Чтение файла.
    Reading,
    /// Поверхности.
    Surfaces,
    /// Пол и потолок.
    Levels,
    /// Стены.
    Walls,
    /// Проёмы и помещения.
    Openings,
    /// Вид сверху.
    Raster,
}

impl Stage {
    /// Номер этапа (для прогресса).
    #[must_use]
    pub fn index(self) -> u32 {
        match self {
            Self::Reading => 0,
            Self::Surfaces => 1,
            Self::Levels => 2,
            Self::Walls => 3,
            Self::Openings => 4,
            Self::Raster => 5,
        }
    }

    /// Всего этапов.
    pub const COUNT: u32 = 6;
}

fn rotate2(v: DVec2, angle: f64) -> DVec2 {
    DVec2::from_angle(angle).rotate(v)
}

/// Сдвиг плана: левый верхний угол стен — в (1000, 1000) мм.
fn place(walls: &[(DVec2, DVec2)]) -> DVec2 {
    let lo = walls
        .iter()
        .flat_map(|(a, b)| [*a, *b])
        .fold(DVec2::splat(f64::MAX), DVec2::min);
    if walls.is_empty() {
        DVec2::ZERO
    } else {
        DVec2::splat(1000.0) - lo
    }
}

/// Распознаёт скан.
///
/// # Errors
///
/// Файл не читается или в нём нет геометрии. Неудача распознавания — не
/// ошибка: она описана в [`ScanResult::failure`].
pub fn analyze(bytes: &[u8], progress: &mut dyn FnMut(Stage)) -> Result<ScanResult, ScanError> {
    progress(Stage::Reading);
    let loaded = io::load(bytes)?;
    let source = ScanSource {
        format: loaded.format.unwrap_or(ScanFormat::Glb),
        triangles: u32::try_from(loaded.mesh.triangles.len()).unwrap_or(u32::MAX),
        points: u32::try_from(loaded.cloud.points.len()).unwrap_or(u32::MAX),
        room_data: loaded.room.is_some(),
    };
    progress(Stage::Surfaces);
    let mut samples = samples::build(&loaded);
    progress(Stage::Levels);
    let levels = detect::levels(&samples);
    if let Some(l) = levels {
        for s in &mut samples {
            s.p.z -= l.floor;
        }
    }
    let levels = levels.map(|l| Levels {
        floor: 0.0,
        ceiling: l.ceiling.map(|c| c - l.floor),
    });

    progress(Stage::Walls);
    let mut result = match (&loaded.room, levels) {
        (Some(room), _) => from_room(room, &mut samples, levels.as_ref(), source),
        (None, Some(levels)) => from_samples(&mut samples, &levels, source),
        (None, None) => ScanResult {
            source,
            ceiling_height: detect::DEFAULT_HEIGHT,
            ceiling_detected: false,
            walls: Vec::new(),
            openings: Vec::new(),
            issues: Vec::new(),
            raster: None,
            failure: Some(ScanFailure::NoFloor),
        },
    };

    progress(Stage::Openings);
    let next = u32::try_from(result.issues.len()).unwrap_or(0);
    let missing = plan::missing_walls(&result, next);
    result.issues.extend(missing);
    if result.failure.is_none() {
        let kept = apply(&result, &[], &ScanParams::default());
        if kept.walls.len() < 3 {
            result.failure = Some(ScanFailure::TooFewWalls);
        }
    }

    progress(Stage::Raster);
    result.raster = raster::render(&samples, levels.as_ref());
    Ok(result)
}

fn from_samples(samples: &mut [Sample], levels: &Levels, source: ScanSource) -> ScanResult {
    let mut angles = detect::orientations(samples, levels);
    // Выравнивание: главное направление стен — по осям плана.
    if let Some(&main) = angles.first() {
        detect::rotate(samples, -main);
        for a in &mut angles {
            *a = (*a - main).rem_euclid(std::f64::consts::PI);
        }
    }
    let faces = detect::faces(samples, levels, &angles);
    let (mut walls, mut openings, mut issues) = assemble::walls(&faces, levels);
    let shift = place(
        &walls
            .iter()
            .map(|w| (w.a.vec(), w.b.vec()))
            .collect::<Vec<_>>(),
    );
    translate(samples, &mut walls, &mut openings, &mut issues, shift);
    ScanResult {
        source,
        ceiling_height: round10(levels.top() - levels.floor),
        ceiling_detected: levels.ceiling.is_some(),
        walls,
        openings,
        issues,
        raster: None,
        failure: None,
    }
}

fn round10(v: f64) -> f64 {
    (v / 10.0).round() * 10.0
}

fn translate(
    samples: &mut [Sample],
    walls: &mut [ScanWall],
    openings: &mut [ScanOpening],
    issues: &mut [ScanIssue],
    shift: DVec2,
) {
    let mv = |p: &mut Point| *p = Point::from_vec(p.vec() + shift);
    for s in samples {
        s.p += shift.extend(0.0);
    }
    for w in walls {
        mv(&mut w.a);
        mv(&mut w.b);
    }
    for o in openings {
        mv(&mut o.center);
    }
    for i in issues {
        mv(&mut i.at);
    }
}

/// Стены и проёмы из параметрической комнаты; выборка (если есть сетка) —
/// для уровней и вида сверху.
fn from_room(
    room: &RoomPlan,
    samples: &mut [Sample],
    levels: Option<&Levels>,
    source: ScanSource,
) -> ScanResult {
    // Координаты RoomPlan: метры, ось +Y вверх — как у сетки GLB.
    let plan = |v: DVec3| DVec2::new(v.z, -v.x) * 1000.0;
    let walls3: Vec<_> = room
        .objects
        .iter()
        .filter(|o| o.kind == RoomKind::Wall)
        .collect();
    let floor_y = walls3
        .iter()
        .map(|w| w.center.y - w.height / 2.0)
        .fold(f64::MAX, f64::min);
    let height = walls3.iter().map(|w| w.height).fold(0.0, f64::max) * 1000.0;
    // Выравнивание по самой длинной стене.
    let angle = walls3
        .iter()
        .max_by(|a, b| a.width.total_cmp(&b.width))
        .map_or(0.0, |w| {
            let d = plan(w.along);
            d.y.atan2(d.x)
        });
    let rot = |v: DVec2| rotate2(v, -angle);
    let mut walls: Vec<ScanWall> = walls3
        .iter()
        .enumerate()
        .map(|(i, w)| {
            let c = rot(plan(w.center));
            let d = rot(plan(w.along)).normalize_or_zero() * (w.width * 500.0);
            ScanWall {
                id: u32::try_from(i).unwrap_or(u32::MAX),
                a: Point::from_vec(c - d),
                b: Point::from_vec(c + d),
                thickness: if w.depth > 0.03 {
                    round10(w.depth * 1000.0)
                } else {
                    150.0
                },
                justification: Justification::Center,
                source: WallSource::RoomPlan,
                confidence: w.confidence,
            }
        })
        .collect();
    let mut openings = Vec::new();
    for o in room.objects.iter().filter(|o| o.kind != RoomKind::Wall) {
        let c = rot(plan(o.center));
        let nearest = walls
            .iter()
            .map(|w| (w.id, op_geom::segment::distance(w.a.vec(), w.b.vec(), c)))
            .filter(|(_, d)| *d <= 400.0)
            .min_by(|a, b| a.1.total_cmp(&b.1));
        let Some((wall, _)) = nearest else {
            continue;
        };
        let w = walls
            .iter()
            .find(|w| w.id == wall)
            .map(|w| (w.a.vec(), w.b.vec()));
        let center = w.map_or(c, |(a, b)| op_geom::segment::closest_point(a, b, c));
        let bottom = (o.center.y - o.height / 2.0 - floor_y).max(0.0) * 1000.0;
        openings.push(ScanOpening {
            id: u32::try_from(openings.len()).unwrap_or(u32::MAX),
            wall,
            kind: match o.kind {
                RoomKind::Window => OpeningKind::Window,
                RoomKind::Opening => OpeningKind::Arch,
                _ => OpeningKind::Door,
            },
            center: Point::from_vec(center),
            width: round10(o.width * 1000.0),
            height: round10(o.height * 1000.0),
            sill: if o.kind == RoomKind::Window {
                round10(bottom)
            } else {
                0.0
            },
            confidence: o.confidence,
        });
    }
    let mut issues: Vec<ScanIssue> = walls
        .iter()
        .filter(|w| w.confidence < 0.5)
        .enumerate()
        .map(|(i, w)| ScanIssue {
            id: u32::try_from(i).unwrap_or(u32::MAX),
            kind: IssueKind::ExtraWall { wall: w.id },
            confidence: 1.0 - w.confidence,
            at: Point::from_vec(w.a.vec().midpoint(w.b.vec())),
        })
        .collect();
    detect::rotate(samples, -angle);
    let shift = place(
        &walls
            .iter()
            .map(|w| (w.a.vec(), w.b.vec()))
            .collect::<Vec<_>>(),
    );
    translate(samples, &mut walls, &mut openings, &mut issues, shift);
    let ceiling = levels.and_then(|l| l.ceiling);
    ScanResult {
        source,
        ceiling_height: round10(ceiling.unwrap_or(if height > 1500.0 {
            height
        } else {
            detect::DEFAULT_HEIGHT
        })),
        ceiling_detected: ceiling.is_some() || height > 1500.0,
        walls,
        openings,
        issues,
        raster: None,
        failure: None,
    }
}

#[cfg(test)]
mod tests;
