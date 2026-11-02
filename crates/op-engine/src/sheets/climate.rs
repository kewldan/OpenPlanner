//! Лист «План тёплых полов и климата»: зоны тёплого пола, климатическое
//! оборудование, таблицы тёплых полов и воздухообмена.

use op_export::{
    Align, Item, Rgb,
    table::{Cell, Column, Table},
};

use super::{LAYER_WALLS, Plot, Views, walls_where};
use crate::protocol::ClimateReport;

/// Слой климата.
const LAYER_CLIMATE: &str = "Климат";

impl Views<'_> {
    /// Климат итогового вида.
    pub(super) fn climate(&self) -> ClimateReport {
        let (doc, derived) = self.final_view();
        crate::climate::report(doc, derived)
    }

    /// Есть ли на итоговом виде тёплые полы или климатическое оборудование.
    pub(super) fn has_climate(&self) -> bool {
        let (doc, derived) = self.final_view();
        !self.climate().zones.is_empty()
            || doc
                .devices
                .iter()
                .any(|d| derived.visible.contains(&d.layer) && d.kind.is_climate())
    }

    /// Таблица тёплых полов.
    pub(super) fn heating_table(&self) -> Option<Table> {
        let (doc, _) = self.final_view();
        let report = self.climate();
        (!report.zones.is_empty()).then(|| Table {
            title: "Тёплые полы".into(),
            columns: vec![
                Column::new("Помещение", 50.0, Align::Left),
                Column::new("S пола, м²", 25.0, Align::Right),
                Column::new("S обогрева, м²", 30.0, Align::Right),
                Column::new("P, Вт", 25.0, Align::Right),
                Column::new("Терморегулятор, H", 55.0, Align::Left),
            ],
            rows: report
                .zones
                .iter()
                .map(|z| {
                    let t = z
                        .thermostat
                        .as_deref()
                        .and_then(|t| doc.device(t))
                        .map_or_else(|| "—".to_owned(), |d| format!("H {:.0}", d.height));
                    vec![
                        Cell::text(z.room_name.clone()),
                        Cell::number(z.area, 2),
                        Cell::number(z.heated, 2),
                        Cell::number(z.power, 0),
                        Cell::text(t),
                    ]
                })
                .collect(),
            footer: Some(vec![
                Cell::text("Итого"),
                Cell::text(""),
                Cell::number(report.zones.iter().map(|z| z.heated).sum(), 2),
                Cell::number(report.zones.iter().map(|z| z.power).sum(), 0),
                Cell::text(""),
            ]),
        })
    }

    /// Таблица воздухообмена и охлаждения.
    pub(super) fn air_table(&self) -> Table {
        let report = self.climate();
        Table {
            title: "Воздухообмен и охлаждение".into(),
            columns: vec![
                Column::new("Помещение", 50.0, Align::Left),
                Column::new("Нужно, м³/ч", 30.0, Align::Right),
                Column::new("Есть, м³/ч", 30.0, Align::Right),
                Column::new("Холод, кВт", 30.0, Align::Right),
                Column::new("Кондиц.", 25.0, Align::Right),
            ],
            rows: report
                .rooms
                .iter()
                .map(|r| {
                    vec![
                        Cell::text(r.name.clone()),
                        Cell::number(r.air_required, 0),
                        Cell::number(r.air_provided, 0),
                        Cell::number(r.cooling, 1),
                        Cell::text(r.ac_units.to_string()),
                    ]
                })
                .collect(),
            footer: None,
        }
    }

    /// План: зоны тёплого пола штриховкой, климатическое оборудование с высотами.
    pub(super) fn draw_climate(&self, plot: &mut Plot<'_>) {
        let (doc, derived) = self.final_view();
        let rooms = match &self.variant {
            Some(_) => self.variant_rooms(),
            None => self.survey_rooms(),
        };
        let layer = plot.sheet.layer(LAYER_CLIMATE);
        for z in &self.climate().zones {
            if let Some(&i) = derived.room_regions.get(&z.room) {
                let region = &derived.regions[i];
                let mut rings = vec![region.contour.clone()];
                rings.extend(region.islands.iter().cloned());
                plot.items
                    .push(Item::fill(layer, rings.clone(), Rgb::hex(0xFBE3D3)));
                super::hatch_lines(
                    &mut plot.items,
                    layer,
                    &rings,
                    45.0,
                    3.0,
                    plot.k,
                    Rgb::hex(0xD9804F),
                );
            }
        }
        plot.walls(LAYER_WALLS, &walls_where(doc, derived, |_| true));
        plot.openings(doc, derived, |_| true);
        plot.room_labels(doc, &rooms, false);
        let climate: Vec<&op_model::Device> = doc
            .devices
            .iter()
            .filter(|d| derived.visible.contains(&d.layer) && d.kind.is_climate())
            .collect();
        plot.devices(&climate, |d| Some(format!("H{:.0}", d.height)));
    }
}
