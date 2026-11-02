//! Импорт распознанного скана: стены, проёмы, помещения, подложка «Скан»
//! и исходный файл — одной правкой.

use op_model::{
    Command, Document, Element, Layer, LayerKind, Node, Opening, Room, Side, Underlay, Wall,
};

use crate::{
    derived::Derived,
    ids::IdGen,
    protocol::{ScanImport, ScanImportMode},
};

fn put(element: Element) -> Command {
    Command::Put {
        element,
        index: None,
    }
}

/// Расширение исходного файла для папки `scan/`.
fn extension(name: &str) -> &'static str {
    let lower = name.to_ascii_lowercase();
    ["glb", "zip", "ply", "obj", "json"]
        .into_iter()
        .find(|e| lower.ends_with(&format!(".{e}")))
        .unwrap_or("bin")
}

/// Команды импорта скана в документ `doc`.
pub fn commands(
    doc: &Document,
    ids: &mut IdGen,
    request: &ScanImport,
    source: Option<Vec<u8>>,
) -> Vec<Command> {
    let mut out = Vec::new();
    let params = &request.params;
    if doc.layer(LayerKind::Scan.id()).is_none() {
        out.push(Command::Put {
            element: Element::Layer(Layer::system(LayerKind::Scan)),
            index: Some(0),
        });
    }
    if (params.underlay || request.mode == ScanImportMode::UnderlayOnly)
        && let Some(raster) = &request.result.raster
    {
        let id = ids.generate();
        let asset = format!("assets/{id}.png");
        out.push(Command::asset(asset.clone(), raster.png.clone()));
        out.push(put(Element::Underlay(Underlay {
            level: None,
            id,
            layer: LayerKind::Scan.id().into(),
            asset,
            name: request.name.clone(),
            x: raster.x,
            y: raster.y,
            width_px: raster.width_px,
            height_px: raster.height_px,
            mm_per_px: raster.mm_per_px,
            opacity: 0.55,
        })));
    }
    if params.keep_source
        && let Some(bytes) = source
    {
        let path = format!("scan/{}.{}", ids.generate(), extension(&request.name));
        out.push(Command::asset(path, bytes));
    }
    if request.mode == ScanImportMode::UnderlayOnly {
        return out;
    }

    if params.ceiling_height.is_finite() && params.ceiling_height > 0.0 {
        out.push(Command::SetDefaultCeilingHeight {
            height: params.ceiling_height,
        });
    }
    let plan = op_scan::apply(&request.result, &request.answers, params);
    let node_ids: Vec<String> = plan
        .nodes
        .iter()
        .map(|p| {
            let id = ids.generate();
            out.push(put(Element::Node(Node {
                level: None,
                variant: None,
                id: id.clone(),
                x: p.x,
                y: p.y,
            })));
            id
        })
        .collect();
    let wall_ids: Vec<String> = plan
        .walls
        .iter()
        .map(|w| {
            let id = ids.generate();
            out.push(put(Element::Wall(Wall {
                material: None,
                variant: None,
                id: id.clone(),
                layer: LayerKind::Walls.id().into(),
                a: node_ids[w.a].clone(),
                b: node_ids[w.b].clone(),
                thickness: w.thickness,
                justification: w.justification,
                height: None,
            })));
            id
        })
        .collect();
    for o in &plan.openings {
        out.push(put(Element::Opening(Opening {
            variant: None,
            id: ids.generate(),
            layer: LayerKind::Openings.id().into(),
            wall: wall_ids[o.wall].clone(),
            kind: o.kind,
            offset: o.offset,
            width: o.width,
            height: o.height,
            sill: o.sill,
            hinge_at_end: false,
            swing: Side::Left,
        })));
    }

    if params.create_rooms {
        // Помещения — по контурам документа с уже добавленными стенами.
        let mut scratch = doc.clone();
        let built = Command::Batch {
            commands: out.clone(),
        }
        .apply(&mut scratch);
        if built.is_ok() {
            let derived = Derived::new(&scratch);
            let used: std::collections::HashSet<usize> =
                derived.room_regions.values().copied().collect();
            let mut number = scratch.rooms.len();
            for (i, region) in derived.regions.iter().enumerate() {
                if used.contains(&i) {
                    continue;
                }
                let Some(p) = region.label_point() else {
                    continue;
                };
                number += 1;
                out.push(put(Element::Room(Room {
                    level: None,
                    variant: None,
                    id: ids.generate(),
                    layer: LayerKind::Rooms.id().into(),
                    name: format!("Помещение {number}"),
                    x: p.x,
                    y: p.y,
                    ceiling_height: None,
                })));
            }
        }
    }
    out
}
