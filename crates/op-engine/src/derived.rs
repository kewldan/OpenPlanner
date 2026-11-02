//! Производные данные документа: контуры стен, области помещений, длины.
//!
//! Пересчитываются целиком при каждом изменении документа — на объёмах
//! одной квартиры это миллисекунды.

use std::collections::{HashMap, HashSet};

use glam::DVec2;
use op_geom::{
    shapes::{self, Region, Shape},
    walls::{WallInput, opening_rect, wall_outlines},
};
use op_model::{Anchor, Document, Opening, Wall};

use crate::protocol::{LengthInfo, RoomInfo};

/// Производные данные.
#[derive(Debug, Clone, Default)]
pub struct Derived {
    /// Положения узлов.
    pub nodes: HashMap<String, DVec2>,
    /// Контур каждой стены со стыками.
    pub wall_outlines: HashMap<String, Vec<DVec2>>,
    /// Стены видимых слоёв для отрисовки: объединение минус проёмы.
    pub wall_shapes: Vec<Shape>,
    /// Замкнутые области стен (все слои).
    pub regions: Vec<Region>,
    /// Область каждого помещения.
    pub room_regions: HashMap<String, usize>,
    /// Видимые слои.
    pub visible: HashSet<String>,
    /// Заблокированные слои.
    pub locked: HashSet<String>,
}

impl Derived {
    /// Считает производные данные документа.
    #[must_use]
    pub fn new(doc: &Document) -> Self {
        let nodes: HashMap<String, DVec2> = doc
            .nodes
            .iter()
            .map(|n| (n.id.clone(), DVec2::new(n.x, n.y)))
            .collect();
        let visible: HashSet<String> = doc
            .layers
            .iter()
            .filter(|l| l.visible)
            .map(|l| l.id.clone())
            .collect();
        let locked: HashSet<String> = doc
            .layers
            .iter()
            .filter(|l| l.locked)
            .map(|l| l.id.clone())
            .collect();

        let index: HashMap<&str, usize> = doc
            .nodes
            .iter()
            .enumerate()
            .map(|(i, n)| (n.id.as_str(), i))
            .collect();
        let points: Vec<DVec2> = doc.nodes.iter().map(|n| DVec2::new(n.x, n.y)).collect();
        let inputs: Vec<WallInput> = doc
            .walls
            .iter()
            .map(|w| {
                let (left, right) = w.sides();
                WallInput {
                    a: index.get(w.a.as_str()).copied().unwrap_or(usize::MAX),
                    b: index.get(w.b.as_str()).copied().unwrap_or(usize::MAX),
                    left,
                    right,
                }
            })
            .collect();
        let outlines = wall_outlines(&points, &inputs);

        let all: Vec<Vec<DVec2>> = outlines.iter().filter(|o| !o.is_empty()).cloned().collect();
        let regions = shapes::regions(&shapes::union(&all));

        let shown: Vec<Vec<DVec2>> = doc
            .walls
            .iter()
            .zip(&outlines)
            .filter(|(w, o)| visible.contains(&w.layer) && !o.is_empty())
            .map(|(_, o)| o.clone())
            .collect();
        let cuts: Vec<Vec<DVec2>> = doc
            .openings
            .iter()
            .filter_map(|o| {
                let wall = doc.wall(&o.wall)?;
                let (a, b) = (nodes.get(&wall.a)?, nodes.get(&wall.b)?);
                let (from, to) = opening_span(o, a.distance(*b));
                let (left, right) = wall.sides();
                Some(opening_rect(*a, *b, left, right, from, to))
            })
            .collect();
        let wall_shapes = shapes::difference(&shown, &cuts);

        let room_regions = doc
            .rooms
            .iter()
            .filter_map(|r| {
                shapes::region_at(&regions, DVec2::new(r.x, r.y)).map(|i| (r.id.clone(), i))
            })
            .collect();

        Self {
            wall_outlines: doc
                .walls
                .iter()
                .zip(outlines)
                .map(|(w, o)| (w.id.clone(), o))
                .collect(),
            nodes,
            wall_shapes,
            regions,
            room_regions,
            visible,
            locked,
        }
    }

    /// Концы оси стены.
    #[must_use]
    pub fn wall_axis(&self, wall: &Wall) -> Option<(DVec2, DVec2)> {
        Some((*self.nodes.get(&wall.a)?, *self.nodes.get(&wall.b)?))
    }

    /// Точка конца размера.
    #[must_use]
    pub fn anchor(&self, anchor: &Anchor) -> Option<DVec2> {
        match anchor {
            Anchor::Node { node } => self.nodes.get(node).copied(),
            Anchor::Point { x, y } => Some(DVec2::new(*x, *y)),
        }
    }

    /// Элемент на слое, который виден и не заблокирован.
    #[must_use]
    pub fn editable(&self, layer: &str) -> bool {
        self.visible.contains(layer) && !self.locked.contains(layer)
    }

    /// Вычисленные величины для UI.
    #[must_use]
    pub fn infos(&self, doc: &Document) -> (Vec<RoomInfo>, Vec<LengthInfo>, Vec<LengthInfo>, u32) {
        let rooms = doc
            .rooms
            .iter()
            .map(|r| {
                let region = self.room_regions.get(&r.id).map(|&i| &self.regions[i]);
                RoomInfo {
                    id: r.id.clone(),
                    closed: region.is_some(),
                    area: region.map_or(0.0, |g| op_geom::mm2_to_m2(g.area)),
                    perimeter: region.map_or(0.0, |g| g.perimeter / 1000.0),
                }
            })
            .collect();
        let walls = doc
            .walls
            .iter()
            .filter_map(|w| {
                let (a, b) = self.wall_axis(w)?;
                Some(LengthInfo {
                    id: w.id.clone(),
                    length: a.distance(b),
                })
            })
            .collect();
        let dimensions = doc
            .dimensions
            .iter()
            .filter_map(|d| {
                Some(LengthInfo {
                    id: d.id.clone(),
                    length: self.anchor(&d.start)?.distance(self.anchor(&d.end)?),
                })
            })
            .collect();
        let used: HashSet<usize> = self.room_regions.values().copied().collect();
        let free = (0..self.regions.len())
            .filter(|i| !used.contains(i))
            .count();
        (
            rooms,
            walls,
            dimensions,
            u32::try_from(free).unwrap_or(u32::MAX),
        )
    }
}

/// Начало и конец проёма вдоль оси стены длиной `len`, мм от узла `a`.
#[must_use]
pub fn opening_span(opening: &Opening, len: f64) -> (f64, f64) {
    let half = opening.width / 2.0;
    let center = opening
        .offset
        .clamp(half.min(len / 2.0), (len - half).max(len / 2.0));
    ((center - half).max(0.0), (center + half).min(len))
}
