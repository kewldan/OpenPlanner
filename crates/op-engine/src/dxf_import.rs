//! Импорт DXF (в том числе выгрузок других планировщиков и CAD): подложкой —
//! линии чертежа растром в точном масштабе — или стенами по линиям выбранных слоёв.

use std::collections::HashMap;

use glam::DVec2;
use op_export::dxf::{DxfDrawing, DxfPath};
use op_model::{Command, Element, Justification, LayerKind, Node, Underlay, Wall};

use crate::{
    protocol::{DxfImport, DxfInfo, DxfLayer, DxfMode, EngineEvents},
    state::EngineState,
};

/// Больше отрезков стенами не импортируется: это уже не план квартиры.
const MAX_WALLS: usize = 5000;
/// Наибольшая сторона растра подложки, пиксели.
const MAX_SIDE: f64 = 4000.0;
/// Концы отрезков ближе этого сливаются в один узел, мм.
const MERGE: f64 = 5.0;

fn read(bytes: &[u8]) -> Result<DxfDrawing, String> {
    let drawing = op_export::dxf::read(bytes).map_err(|e| e.to_string())?;
    if drawing.paths.is_empty() {
        return Err("в чертеже нет линий: отрезков, полилиний, дуг".into());
    }
    Ok(drawing)
}

fn bounds<'a>(paths: impl Iterator<Item = &'a DxfPath>) -> (DVec2, DVec2) {
    paths.flat_map(|p| p.points.iter()).fold(
        (DVec2::splat(f64::INFINITY), DVec2::splat(f64::NEG_INFINITY)),
        |(lo, hi), p| (lo.min(*p), hi.max(*p)),
    )
}

/// Слои чертежа с числом линий и габарит, мм.
///
/// # Errors
///
/// Файл не читается как текстовый DXF или в нём нет линий.
pub fn info(bytes: &[u8]) -> Result<DxfInfo, String> {
    let d = read(bytes)?;
    let (lo, hi) = bounds(d.paths.iter());
    Ok(DxfInfo {
        layers: d
            .layers
            .iter()
            .map(|name| DxfLayer {
                name: name.clone(),
                paths: u32::try_from(d.paths.iter().filter(|p| &p.layer == name).count())
                    .unwrap_or(u32::MAX),
            })
            .collect(),
        width: hi.x - lo.x,
        height: hi.y - lo.y,
    })
}

/// Отрезки линий (у замкнутых — и замыкающий).
fn segments(paths: &[&DxfPath]) -> Vec<(DVec2, DVec2)> {
    let mut out = Vec::new();
    for p in paths {
        for w in p.points.windows(2) {
            out.push((w[0], w[1]));
        }
        if p.closed
            && let (Some(a), Some(b)) = (p.points.last(), p.points.first())
        {
            out.push((*a, *b));
        }
    }
    out.retain(|(a, b)| a.distance(*b) > MERGE);
    out
}

/// Растр линий: тёмные линии толщиной 2 пикселя на прозрачном фоне.
fn raster(segments: &[(DVec2, DVec2)], lo: DVec2, mm_per_px: f64, w: u32, h: u32) -> Vec<u8> {
    let (wi, hi) = (w as usize, h as usize);
    let mut rgba = vec![0u8; wi * hi * 4];
    let mut dot = |x: f64, y: f64| {
        for (dx, dy) in [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)] {
            let (px, py) = ((x + dx).floor(), (y + dy).floor());
            if px >= 0.0 && py >= 0.0 && px < f64::from(w) && py < f64::from(h) {
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // проверено выше
                let at = (py as usize * wi + px as usize) * 4;
                rgba[at..at + 4].copy_from_slice(&[35, 35, 31, 255]);
            }
        }
    };
    for (a, b) in segments {
        let (pa, pb) = ((*a - lo) / mm_per_px, (*b - lo) / mm_per_px);
        let steps = pa.distance(pb).ceil().max(1.0);
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        // шагов ≤ сторона растра × 2
        let n = steps as u32;
        for i in 0..=n {
            let p = pa.lerp(pb, f64::from(i) / steps);
            dot(p.x, p.y);
        }
    }
    rgba
}

fn png(rgba: &[u8], w: u32, h: u32) -> Result<Vec<u8>, String> {
    let image = image::RgbaImage::from_raw(w, h, rgba.to_vec()).ok_or("растр подложки")?;
    let mut out = std::io::Cursor::new(Vec::new());
    image
        .write_to(&mut out, image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    Ok(out.into_inner())
}

impl EngineState {
    /// Импортирует DXF подложкой или стенами одним действием.
    ///
    /// # Errors
    ///
    /// Файл не читается, на выбранных слоях нет линий или их слишком много.
    pub fn import_dxf(
        &mut self,
        bytes: &[u8],
        name: &str,
        options: &DxfImport,
    ) -> Result<EngineEvents, String> {
        let d = read(bytes)?;
        let chosen: Vec<&DxfPath> = d
            .paths
            .iter()
            .filter(|p| options.layers.is_empty() || options.layers.contains(&p.layer))
            .collect();
        let segs = segments(&chosen);
        if segs.is_empty() {
            return Err("на выбранных слоях нет линий".into());
        }
        let events = match options.mode {
            DxfMode::Underlay => {
                let (lo, hi) = bounds(chosen.iter().copied());
                let size = (hi - lo).max(DVec2::splat(1.0));
                let mm_per_px = (size.max_element() / MAX_SIDE).max(1.0);
                // Поле в 4 пикселя вокруг линий.
                let lo = lo - DVec2::splat(4.0 * mm_per_px);
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                // ≤ MAX_SIDE + поле
                let (w, h) = (
                    (size.x / mm_per_px).ceil() as u32 + 8,
                    (size.y / mm_per_px).ceil() as u32 + 8,
                );
                let bytes = png(&raster(&segs, lo, mm_per_px, w, h), w, h)?;
                let decoded = crate::decode_image(&bytes)?;
                let id = self.ids.generate();
                let asset = format!("assets/{id}.png");
                self.images_insert(asset.clone(), decoded);
                let underlay = Underlay {
                    level: None,
                    id: id.clone(),
                    layer: LayerKind::Underlays.id().into(),
                    asset: asset.clone(),
                    name: name.to_owned(),
                    x: lo.x,
                    y: lo.y,
                    width_px: w,
                    height_px: h,
                    mm_per_px,
                    opacity: 0.8,
                };
                let mut events = self.execute_all(vec![
                    Command::asset(asset, bytes),
                    Command::Put {
                        element: Element::Underlay(underlay),
                        index: None,
                    },
                ]);
                events.extend(self.select(vec![id]));
                events
            }
            DxfMode::Walls => {
                if segs.len() > MAX_WALLS {
                    return Err(format!(
                        "на выбранных слоях {} отрезков — оставьте только слой стен",
                        segs.len()
                    ));
                }
                if !(options.thickness.is_finite() && options.thickness > 0.0) {
                    return Err("толщина стен должна быть больше нуля".into());
                }
                let mut nodes: Vec<(String, DVec2)> = Vec::new();
                let mut index: HashMap<(i64, i64), usize> = HashMap::new();
                let mut commands = Vec::new();
                let mut node_at = |p: DVec2, state: &mut Self, commands: &mut Vec<Command>| {
                    // Узлы сливаются по сетке MERGE мм (и по соседним ячейкам).
                    #[allow(clippy::cast_possible_truncation)] // координаты квартиры
                    let key = ((p.x / MERGE).round() as i64, (p.y / MERGE).round() as i64);
                    for dx in -1..=1 {
                        for dy in -1..=1 {
                            if let Some(&i) = index.get(&(key.0 + dx, key.1 + dy))
                                && nodes[i].1.distance(p) <= MERGE
                            {
                                return nodes[i].0.clone();
                            }
                        }
                    }
                    let id = state.ids.generate();
                    commands.push(Command::Put {
                        element: Element::Node(Node {
                            level: None,
                            id: id.clone(),
                            variant: None,
                            x: (p.x * 10.0).round() / 10.0,
                            y: (p.y * 10.0).round() / 10.0,
                        }),
                        index: None,
                    });
                    index.insert(key, nodes.len());
                    nodes.push((id.clone(), p));
                    id
                };
                let mut seen = std::collections::HashSet::new();
                for (a, b) in &segs {
                    let na = node_at(*a, self, &mut commands);
                    let nb = node_at(*b, self, &mut commands);
                    let pair = if na < nb {
                        (na.clone(), nb.clone())
                    } else {
                        (nb.clone(), na.clone())
                    };
                    if na == nb || !seen.insert(pair) {
                        continue;
                    }
                    commands.push(Command::Put {
                        element: Element::Wall(Wall {
                            id: self.ids.generate(),
                            layer: LayerKind::Walls.id().into(),
                            variant: None,
                            a: na,
                            b: nb,
                            thickness: options.thickness,
                            justification: Justification::Center,
                            height: None,
                            material: None,
                        }),
                        index: None,
                    });
                }
                let events = self.execute_all(commands);
                self.zoom_to_fit_now();
                events
            }
        };
        Ok(EngineEvents { events })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROOM: &str = "0\nSECTION\n2\nHEADER\n9\n$INSUNITS\n70\n4\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n0\nLWPOLYLINE\n8\nСтены\n90\n4\n70\n1\n10\n0\n20\n0\n10\n4000\n20\n0\n10\n4000\n20\n3000\n10\n0\n20\n3000\n0\nLINE\n8\nМебель\n10\n100\n20\n100\n11\n900\n21\n100\n0\nENDSEC\n0\nEOF\n";

    #[test]
    fn layers_and_size() {
        let i = info(ROOM.as_bytes()).unwrap();
        assert_eq!(i.layers.len(), 2);
        assert_eq!(i.layers[0].name, "Стены");
        assert!((i.width - 4000.0).abs() < 1e-9 && (i.height - 3000.0).abs() < 1e-9);
    }

    #[test]
    fn walls_from_a_layer_close_into_a_room() {
        let mut s = EngineState::new(DVec2::new(1000.0, 800.0), 1.0);
        let options = DxfImport {
            mode: DxfMode::Walls,
            layers: vec!["Стены".into()],
            thickness: 200.0,
        };
        s.import_dxf(ROOM.as_bytes(), "план.dxf", &options).unwrap();
        assert_eq!(s.document().walls.len(), 4);
        assert_eq!(s.document().nodes.len(), 4);
        s.handle(crate::protocol::ToEngine::FindRooms);
        assert_eq!(s.document().rooms.len(), 1);
        // Одно действие истории.
        s.handle(crate::protocol::ToEngine::Undo);
        s.handle(crate::protocol::ToEngine::Undo);
        assert!(s.document().walls.is_empty());
    }

    #[test]
    fn underlay_keeps_the_scale() {
        let mut s = EngineState::new(DVec2::new(1000.0, 800.0), 1.0);
        let options = DxfImport {
            mode: DxfMode::Underlay,
            layers: Vec::new(),
            thickness: 0.0,
        };
        s.import_dxf(ROOM.as_bytes(), "план.dxf", &options).unwrap();
        let u = &s.document().underlays[0];
        assert!((u.mm_per_px - 1.0).abs() < 1e-9, "{u:?}");
        // Левый верхний угол — на 4 пикселя левее и выше чертежа (Y вниз: −3000).
        assert!(
            (u.x + 4.0).abs() < 1e-9 && (u.y + 3004.0).abs() < 1e-9,
            "{u:?}"
        );
        assert!(s.document().assets.contains_key(&u.asset));
        let bad = DxfImport {
            mode: DxfMode::Walls,
            layers: vec!["нет".into()],
            thickness: 100.0,
        };
        assert!(s.import_dxf(ROOM.as_bytes(), "x", &bad).is_err());
    }
}
