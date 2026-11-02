//! Булевы операции над контурами: объединение стен, вырезы проёмов,
//! области помещений.
//!
//! Используется `i_overlay` (MIT).

use glam::DVec2;
use i_overlay::{
    core::{fill_rule::FillRule, overlay_rule::OverlayRule},
    float::{overlay::FloatOverlay, single::SingleFloatOverlay},
};

use crate::{area, contains, interior_point, perimeter};

/// Фигура: внешний контур и отверстия.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Shape {
    /// Внешний контур.
    pub outer: Vec<DVec2>,
    /// Отверстия.
    pub holes: Vec<Vec<DVec2>>,
}

/// Замкнутая область, окружённая стенами.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Region {
    /// Контур по поверхностям стен.
    pub contour: Vec<DVec2>,
    /// Острова внутри области (отдельно стоящие стены, колонны).
    pub islands: Vec<Vec<DVec2>>,
    /// Площадь пола без островов, мм².
    pub area: f64,
    /// Периметр по контуру, мм.
    pub perimeter: f64,
}

impl Region {
    /// Лежит ли точка внутри области (и не на острове).
    #[must_use]
    pub fn contains(&self, p: DVec2) -> bool {
        contains(&self.contour, p) && !self.islands.iter().any(|i| contains(i, p))
    }

    /// Точка внутри области — для подписи помещения.
    #[must_use]
    pub fn label_point(&self) -> Option<DVec2> {
        interior_point(&self.contour, &self.islands)
    }
}

fn to_raw(contours: &[Vec<DVec2>]) -> Vec<Vec<[f64; 2]>> {
    contours
        .iter()
        .filter(|c| c.len() >= 3)
        .map(|c| c.iter().map(|p| [p.x, p.y]).collect())
        .collect()
}

fn from_raw(contour: Vec<[f64; 2]>) -> Vec<DVec2> {
    contour.into_iter().map(|[x, y]| DVec2::new(x, y)).collect()
}

fn to_shapes(raw: Vec<Vec<Vec<[f64; 2]>>>) -> Vec<Shape> {
    raw.into_iter()
        .filter_map(|shape| {
            let mut contours = shape.into_iter().map(from_raw);
            let outer = contours.next()?;
            Some(Shape {
                outer,
                holes: contours.collect(),
            })
        })
        .collect()
}

/// Объединение контуров (правило ненулевого обхода).
#[must_use]
pub fn union(contours: &[Vec<DVec2>]) -> Vec<Shape> {
    let subj = to_raw(contours);
    if subj.is_empty() {
        return Vec::new();
    }
    to_shapes(FloatOverlay::with_subj(&subj).overlay(OverlayRule::Subject, FillRule::NonZero))
}

/// Объединение `subject` минус объединение `cut`.
#[must_use]
pub fn difference(subject: &[Vec<DVec2>], cut: &[Vec<DVec2>]) -> Vec<Shape> {
    let subj = to_raw(subject);
    if subj.is_empty() {
        return Vec::new();
    }
    let clip = to_raw(cut);
    if clip.is_empty() {
        return union(subject);
    }
    to_shapes(subj.overlay(&clip, OverlayRule::Difference, FillRule::NonZero))
}

/// Замкнутые области, окружённые стенами: отверстия в объединении стен.
///
/// Фигуры, попавшие внутрь отверстия, считаются островами этой области.
#[must_use]
pub fn regions(walls: &[Shape]) -> Vec<Region> {
    let mut regions: Vec<Region> = walls
        .iter()
        .flat_map(|s| s.holes.iter())
        .map(|hole| Region {
            contour: hole.clone(),
            islands: Vec::new(),
            area: area(hole),
            perimeter: perimeter(hole),
        })
        .collect();
    for shape in walls {
        let Some(&probe) = shape.outer.first() else {
            continue;
        };
        // Самая маленькая область, содержащая фигуру, — её непосредственный родитель.
        let parent = regions
            .iter_mut()
            .filter(|r| contains(&r.contour, probe))
            .min_by(|x, y| area(&x.contour).total_cmp(&area(&y.contour)));
        if let Some(parent) = parent {
            parent.area -= area(&shape.outer);
            parent.islands.push(shape.outer.clone());
        }
    }
    regions
}

/// Индекс самой маленькой области, содержащей точку.
#[must_use]
pub fn region_at(regions: &[Region], p: DVec2) -> Option<usize> {
    regions
        .iter()
        .enumerate()
        .filter(|(_, r)| r.contains(p))
        .min_by(|(_, x), (_, y)| area(&x.contour).total_cmp(&area(&y.contour)))
        .map(|(i, _)| i)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::walls::{WallInput, opening_rect, wall_outlines};

    /// Прямоугольная комната по осям стен w × h с толщиной t.
    fn box_room(w: f64, h: f64, t: f64) -> Vec<Vec<DVec2>> {
        let nodes = [
            DVec2::ZERO,
            DVec2::new(w, 0.0),
            DVec2::new(w, h),
            DVec2::new(0.0, h),
        ];
        let walls: Vec<WallInput> = (0..4)
            .map(|i| WallInput {
                a: i,
                b: (i + 1) % 4,
                left: t / 2.0,
                right: t / 2.0,
            })
            .collect();
        wall_outlines(&nodes, &walls)
    }

    #[test]
    fn closed_box_gives_room_area_by_inner_faces() {
        let shapes = union(&box_room(3000.0, 4000.0, 100.0));
        assert_eq!(shapes.len(), 1);
        let found = regions(&shapes);
        assert_eq!(found.len(), 1);
        assert!((crate::mm2_to_m2(found[0].area) - 2.9 * 3.9).abs() < 1e-6);
        assert!((found[0].perimeter - 2.0 * (2900.0 + 3900.0)).abs() < 1e-6);
        assert!(region_at(&found, DVec2::new(1500.0, 2000.0)).is_some());
        assert!(region_at(&found, DVec2::new(5000.0, 2000.0)).is_none());
    }

    #[test]
    fn open_box_has_no_room() {
        let mut walls = box_room(3000.0, 4000.0, 100.0);
        walls.pop();
        assert!(regions(&union(&walls)).is_empty());
    }

    #[test]
    fn partition_splits_room_with_t_junctions() {
        let mut walls = box_room(4000.0, 3000.0, 100.0);
        // Перегородка от середины верхней стены до середины нижней: Т-стыки без разбиения стен.
        let nodes = [DVec2::new(2000.0, 0.0), DVec2::new(2000.0, 3000.0)];
        walls.extend(wall_outlines(
            &nodes,
            &[WallInput {
                a: 0,
                b: 1,
                left: 50.0,
                right: 50.0,
            }],
        ));
        let found = regions(&union(&walls));
        assert_eq!(found.len(), 2);
        for region in &found {
            assert!((crate::mm2_to_m2(region.area) - 1.9 * 2.9).abs() < 1e-6);
        }
    }

    #[test]
    fn column_is_an_island() {
        let mut walls = box_room(4000.0, 4000.0, 100.0);
        walls.push(vec![
            DVec2::new(1900.0, 1900.0),
            DVec2::new(2100.0, 1900.0),
            DVec2::new(2100.0, 2100.0),
            DVec2::new(1900.0, 2100.0),
        ]);
        let found = regions(&union(&walls));
        assert_eq!(found.len(), 1);
        assert!((found[0].area - (3900.0 * 3900.0 - 200.0 * 200.0)).abs() < 1e-3);
        assert!(!found[0].contains(DVec2::new(2000.0, 2000.0)));
        let label = found[0].label_point().unwrap();
        assert!(found[0].contains(label));
    }

    #[test]
    fn opening_cuts_wall() {
        let a = DVec2::ZERO;
        let b = DVec2::new(3000.0, 0.0);
        let wall = wall_outlines(
            &[a, b],
            &[WallInput {
                a: 0,
                b: 1,
                left: 50.0,
                right: 50.0,
            }],
        );
        let cut = opening_rect(a, b, 50.0, 50.0, 1000.0, 1900.0);
        let pieces = difference(&wall, &[cut]);
        assert_eq!(pieces.len(), 2);
        let total: f64 = pieces.iter().map(|s| area(&s.outer)).sum();
        assert!((total - 2100.0 * 100.0).abs() < 1e-3);
    }
}
