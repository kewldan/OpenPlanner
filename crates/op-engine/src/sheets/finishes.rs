//! Листы отделки: план полов с раскладкой и план потолков, ведомость отделки
//! помещений и расход материалов.

use op_export::{
    Align, Item, Rgb,
    table::{Cell, Column, THIN, Table},
};
use op_geom::tiling;
use op_model::{Finish, Surface};

use super::{LAYER_WALLS, Plot, Views, walls_where};
use crate::{
    finish::{self, room_geometry},
    protocol::{FinishReport, Quantity},
};

/// Слой отделки.
const LAYER_FINISH: &str = "Отделка";

/// «120 шт. (15 уп.)».
pub(super) fn quantity(q: &Quantity) -> String {
    let whole = matches!(q.unit.as_str(), "шт." | "рул.");
    let amount = op_export::table::format_number(q.amount, if whole { 0 } else { 1 });
    let pack = match q.name.as_str() {
        "Клей" => "меш.",
        "Краска" => "банок",
        _ => "уп.",
    };
    if q.packs > 0 {
        format!("{} {amount} {} ({} {pack})", q.name, q.unit, q.packs)
    } else {
        format!("{} {amount} {}", q.name, q.unit)
    }
}

fn hex(color: &str) -> Rgb {
    let h = color.trim_start_matches('#');
    u32::from_str_radix(h, 16)
        .ok()
        .filter(|_| h.len() == 6)
        .map_or(Rgb::hex(0xDDDDDD), Rgb::hex)
}

impl Views<'_> {
    /// Расчёт отделки итогового вида.
    pub(super) fn finish_report(&self) -> FinishReport {
        let (doc, derived) = self.final_view();
        finish::report(doc, derived)
    }

    /// Отделки итогового вида на поверхности `surface` (помещения вида, есть площадь).
    fn finishes(&self, surface: Option<Surface>) -> Vec<&Finish> {
        let (doc, _) = self.final_view();
        doc.finishes
            .iter()
            .filter(|f| doc.room(&f.room).is_some())
            .filter(|f| surface.is_none_or(|s| f.surface == s))
            .collect()
    }

    /// Есть ли отделка поверхности на итоговом виде.
    pub(super) fn has_finishes(&self, surface: Surface) -> bool {
        !self.finishes(Some(surface)).is_empty()
    }

    /// Ведомость отделки: помещение, поверхность, материал, площадь, расход.
    pub(super) fn finishes_table(&self, surface: Option<Surface>) -> Table {
        let (doc, derived) = self.final_view();
        let report = self.finish_report();
        let rows = self
            .finishes(surface)
            .into_iter()
            .filter_map(|f| {
                let info = report.finishes.iter().find(|i| i.id == f.id)?;
                if info.area <= 0.0 {
                    return None;
                }
                let room = doc.room(&f.room)?;
                let faces = room_geometry(doc, derived, &f.room)
                    .map(|g| g.faces)
                    .unwrap_or_default();
                let mut place = finish::surface_name(f, &faces);
                if f.surface == Surface::Walls && (f.bottom > 0.0 || f.top.is_some()) {
                    place.push_str(&format!(
                        ", {}–{}",
                        f.bottom.round(),
                        f.top
                            .map_or_else(|| "потолок".to_owned(), |t| t.round().to_string())
                    ));
                }
                let material = finish::material(doc, &f.material)
                    .map_or_else(|| "—".to_owned(), |m| m.name.clone());
                Some(vec![
                    Cell::text(room.name.clone()),
                    Cell::text(place),
                    Cell::text(material),
                    Cell::number(info.area, 2),
                    Cell::text(
                        info.quantities
                            .iter()
                            .map(quantity)
                            .collect::<Vec<_>>()
                            .join("; "),
                    ),
                ])
            })
            .collect();
        Table {
            title: match surface {
                Some(Surface::Floor) => "Ведомость отделки полов".into(),
                Some(Surface::Ceiling) => "Ведомость отделки потолков".into(),
                _ => "Ведомость отделки помещений".into(),
            },
            columns: vec![
                Column::new("Помещение", 30.0, Align::Left),
                Column::new("Поверхность", 30.0, Align::Left),
                Column::new("Материал", 50.0, Align::Left),
                Column::new("S, м²", 15.0, Align::Right),
                Column::new("Расход", 60.0, Align::Left),
            ],
            rows,
            footer: None,
        }
    }

    /// Расход материалов по проекту.
    pub(super) fn totals_table(&self) -> Option<Table> {
        let report = self.finish_report();
        (!report.totals.is_empty()).then(|| Table {
            title: "Расход отделочных материалов".into(),
            columns: vec![
                Column::new("Материал", 70.0, Align::Left),
                Column::new("S, м²", 20.0, Align::Right),
                Column::new("Количество", 95.0, Align::Left),
            ],
            rows: report
                .totals
                .iter()
                .map(|t| {
                    vec![
                        Cell::text(t.name.clone()),
                        Cell::number(t.area, 2),
                        Cell::text(
                            t.quantities
                                .iter()
                                .map(quantity)
                                .collect::<Vec<_>>()
                                .join("; "),
                        ),
                    ]
                })
                .collect(),
            footer: None,
        })
    }

    /// План полов (раскладка) или потолков: заливка цветом материала.
    pub(super) fn draw_finishes(&self, surface: Surface, plot: &mut Plot<'_>) {
        let (doc, derived) = self.final_view();
        let rooms = match &self.variant {
            Some(_) => self.variant_rooms(),
            None => self.survey_rooms(),
        };
        let layer = plot.sheet.layer(LAYER_FINISH);
        for f in self.finishes(Some(surface)) {
            // Действует последняя по списку отделка поверхности помещения.
            let last = doc
                .finishes
                .iter()
                .rev()
                .find(|x| x.room == f.room && x.surface == surface)
                .is_some_and(|x| x.id == f.id);
            let (Some(room), Some(material)) = (
                room_geometry(doc, derived, &f.room),
                finish::material(doc, &f.material),
            ) else {
                continue;
            };
            if !last {
                continue;
            }
            let mut rings = vec![room.contour.clone()];
            rings.extend(room.islands.iter().cloned());
            plot.items
                .push(Item::fill(layer, rings, hex(&material.color)));
            if surface == Surface::Floor {
                let (areas, origin) = finish::areas(doc, f, &room);
                if let Some(spec) = finish::spec(&material, f, origin) {
                    let layout = tiling::layout_all(&areas, &spec, true);
                    if !layout.approximate {
                        for p in layout.pieces {
                            plot.items.push(Item::polyline(
                                layer,
                                p.outline,
                                true,
                                THIN * 0.7,
                                if p.narrow {
                                    Rgb::hex(0xC2410C)
                                } else {
                                    Rgb::hex(0x666666)
                                },
                            ));
                        }
                    }
                }
            }
        }
        plot.walls(LAYER_WALLS, &walls_where(doc, derived, |_| true));
        plot.openings(doc, derived, |_| true);
        plot.room_labels(doc, &rooms, false);
    }
}
