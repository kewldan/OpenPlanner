//! Модель здания для экспорта в IFC 4: стены с проёмами, двери и окна,
//! помещения с площадью, мебель габаритами — по итоговому виду (вариант или обмер).

use op_export::ifc::{IfcBox, IfcModel, IfcOpening, IfcSpace, IfcStorey, IfcWall, OpeningFill};
use op_model::{Document, OpeningKind, view::project};

use crate::{derived::Derived, geometry};

/// Этаж модели из проекции документа.
fn storey(doc: &Document, derived: &Derived, key: &str, name: &str, elevation: f64) -> IfcStorey {
    let shown = |layer: &str| derived.visible.contains(layer);
    let ceiling = doc.default_ceiling_height;
    let walls = doc
        .walls
        .iter()
        .filter(|w| shown(&w.layer))
        .filter_map(|w| {
            let outline = derived.wall_outlines.get(&w.id)?.clone();
            (outline.len() >= 3).then(|| IfcWall {
                key: w.id.clone(),
                name: if w.variant.is_some() {
                    "Перегородка".into()
                } else {
                    "Стена".into()
                },
                outline,
                height: w.height.unwrap_or(ceiling),
                openings: doc
                    .openings
                    .iter()
                    .filter(|o| o.wall == w.id && shown(&o.layer))
                    .filter_map(|o| {
                        let g = geometry::opening(doc, derived, o)?;
                        // Вырез чуть толще стены, чтобы не оставлять плёнку на гранях.
                        let pad = g.normal * 5.0;
                        let outline = vec![
                            g.start + g.normal * g.left + pad,
                            g.end + g.normal * g.left + pad,
                            g.end - g.normal * g.right - pad,
                            g.start - g.normal * g.right - pad,
                        ];
                        let sill = if o.kind == OpeningKind::Window {
                            o.sill
                        } else {
                            0.0
                        };
                        Some(IfcOpening {
                            key: o.id.clone(),
                            outline,
                            sill,
                            height: o.height,
                            width: o.width,
                            fill: match o.kind {
                                OpeningKind::Door => OpeningFill::Door,
                                OpeningKind::Window => OpeningFill::Window,
                                OpeningKind::Arch => OpeningFill::None,
                            },
                        })
                    })
                    .collect(),
            })
        })
        .collect();
    let mut used = std::collections::HashSet::new();
    let spaces = doc
        .rooms
        .iter()
        .filter(|r| shown(&r.layer))
        .filter_map(|r| {
            let i = *derived.room_regions.get(&r.id)?;
            used.insert(i).then_some((r, i))
        })
        .enumerate()
        .map(|(n, (r, i))| {
            let region = &derived.regions[i];
            IfcSpace {
                key: r.id.clone(),
                name: (n + 1).to_string(),
                long_name: r.name.clone(),
                outline: region.contour.clone(),
                height: r.ceiling_height.unwrap_or(ceiling),
                area: op_geom::mm2_to_m2(region.area),
            }
        })
        .collect();
    let furniture = doc
        .furniture
        .iter()
        .filter(|f| shown(&f.layer))
        .map(|f| IfcBox {
            key: f.id.clone(),
            name: f.name.clone(),
            outline: crate::furniture::corners(f),
            bottom: f.elevation,
            height: f.height,
        })
        .collect();
    IfcStorey {
        key: key.to_owned(),
        name: name.to_owned(),
        elevation,
        walls,
        spaces,
        furniture,
    }
}

/// Дата основной надписи «ММ.ГГ» → «20ГГ-ММ-01T00:00:00».
fn timestamp(date: &str) -> String {
    match date.split_once('.') {
        Some((m, y)) if m.len() == 2 && y.len() == 2 => format!("20{y}-{m}-01T00:00:00"),
        _ => "2026-01-01T00:00:00".into(),
    }
}

/// IFC 4 итогового вида: вариант или (без варианта) обмер.
#[must_use]
pub fn export(doc: &Document, variant: Option<&str>, date: &str, file_name: &str) -> Vec<u8> {
    let projected = project(doc, variant);
    let derived = Derived::new(&projected);
    let model = IfcModel {
        project: if doc.meta.title.trim().is_empty() {
            "OpenPlanner".into()
        } else {
            doc.meta.title.clone()
        },
        address: doc.meta.address.clone(),
        timestamp: timestamp(date),
        storeys: vec![storey(&projected, &derived, "main", "Этаж 1", 0.0)],
    };
    op_export::ifc::write(&model, file_name).into_bytes()
}
