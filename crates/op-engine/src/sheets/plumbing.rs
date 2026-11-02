//! Листы сантехники: план водоснабжения (В1, Т3) и план канализации (К1) с
//! ведомостью выводов, трубами и спецификацией труб и фитингов.

use glam::DVec2;
use op_export::{
    Align, Item, Rgb,
    sheet::Shape as Mark,
    table::{Cell, Column, THIN, Table},
};
use op_model::{Document, PipeSystem, PlumbingKind, PlumbingPoint};

use super::{LAYER_FURNITURE, LAYER_WALLS, Plot, SMALL_HEIGHT, SYMBOL_LINE, Views, walls_where};
use crate::{plumbing, protocol::PlumbingReport};

/// Слой листов сантехники.
const LAYER_PLUMBING: &str = "Сантехника";

/// Цвет и штрих системы на листе.
fn pen(system: PipeSystem) -> (Rgb, Option<(f64, f64)>) {
    match system {
        PipeSystem::Cold => (Rgb::hex(0x1D4ED8), None),
        PipeSystem::Hot => (Rgb::hex(0xC81E1E), Some((4.0, 1.0))),
        PipeSystem::Drain => (Rgb::hex(0x6B4A2B), Some((2.0, 1.0))),
    }
}

impl Views<'_> {
    /// Итоговый вид только с системами водоснабжения (`water`) или канализации.
    fn plumbing_doc(&self, water: bool) -> Document {
        let (doc, _) = self.final_view();
        let mut doc = doc.clone();
        doc.plumbing.retain(|p| p.system.is_water() == water);
        doc.pipes.retain(|p| p.system.is_water() == water);
        doc
    }

    /// Расчёт сантехники итогового вида по водоснабжению или канализации.
    pub(super) fn plumbing_report(&self, water: bool) -> PlumbingReport {
        let (_, derived) = self.final_view();
        plumbing::report(&self.plumbing_doc(water), derived)
    }

    /// Есть ли на итоговом виде точки водоснабжения (`water`) или канализации.
    pub(super) fn has_plumbing(&self, water: bool) -> bool {
        let (doc, derived) = self.final_view();
        doc.plumbing
            .iter()
            .any(|p| derived.visible.contains(&p.layer) && p.system.is_water() == water)
    }

    fn plumbing_points(&self, water: bool) -> Vec<&PlumbingPoint> {
        let (doc, derived) = self.final_view();
        doc.plumbing
            .iter()
            .filter(|p| derived.visible.contains(&p.layer) && p.system.is_water() == water)
            .collect()
    }

    /// Ведомость выводов, стояков и коллекторов.
    pub(super) fn plumbing_points_table(&self, water: bool) -> Table {
        let mut rows: Vec<(String, String, f64, f64, usize)> = Vec::new();
        for p in self.plumbing_points(water) {
            let name = plumbing::point_name(p);
            let label = if p.kind == PlumbingKind::Outlet {
                p.label.clone()
            } else {
                String::new()
            };
            let h = p.height.round();
            match rows
                .iter_mut()
                .find(|r| r.0 == name && r.1 == label && (r.2 - h).abs() < 0.5)
            {
                Some(r) => r.4 += 1,
                None => rows.push((name, label, h, p.diameter, 1)),
            }
        }
        Table {
            title: if water {
                "Ведомость выводов водоснабжения".into()
            } else {
                "Ведомость выводов канализации".into()
            },
            columns: vec![
                Column::new("Точка", 55.0, Align::Left),
                Column::new("Прибор", 70.0, Align::Left),
                Column::new("H, мм", 20.0, Align::Right),
                Column::new("Ø, мм", 20.0, Align::Right),
                Column::new("Кол.", 20.0, Align::Right),
            ],
            rows: rows
                .into_iter()
                .map(|r| {
                    vec![
                        Cell::text(r.0),
                        Cell::text(r.1),
                        Cell::number(r.2, 0),
                        Cell::number(r.3, 0),
                        Cell::text(r.4.to_string()),
                    ]
                })
                .collect(),
            footer: None,
        }
    }

    /// Трубы канализации: длины, уклоны и замечания.
    pub(super) fn drain_pipes_table(&self) -> Option<Table> {
        let report = self.plumbing_report(false);
        (!report.pipes.is_empty()).then(|| Table {
            title: "Трубы канализации".into(),
            columns: vec![
                Column::new("Прибор", 45.0, Align::Left),
                Column::new("Ø, мм", 18.0, Align::Right),
                Column::new("L, м", 18.0, Align::Right),
                Column::new("Уклон, %", 22.0, Align::Right),
                Column::new("Замечания", 82.0, Align::Left),
            ],
            rows: report
                .pipes
                .iter()
                .map(|p| {
                    vec![
                        Cell::text(if p.label.is_empty() {
                            "—".to_owned()
                        } else {
                            p.label.clone()
                        }),
                        Cell::number(p.diameter, 0),
                        Cell::number(p.length, 2),
                        p.slope
                            .map_or_else(|| Cell::text("—"), |s| Cell::number(s * 100.0, 1)),
                        Cell::text(if p.warnings.is_empty() {
                            "—".to_owned()
                        } else {
                            p.warnings.join("; ")
                        }),
                    ]
                })
                .collect(),
            footer: None,
        })
    }

    /// Спецификация труб и фитингов.
    pub(super) fn bill_table(&self, water: bool) -> Option<Table> {
        let report = self.plumbing_report(water);
        (!report.bill.is_empty()).then(|| Table {
            title: if water {
                "Спецификация: водоснабжение".into()
            } else {
                "Спецификация: канализация".into()
            },
            columns: vec![
                Column::new("Наименование", 95.0, Align::Left),
                Column::new("Ед.", 15.0, Align::Center),
                Column::new("Кол.", 22.0, Align::Right),
                Column::new("Примечание", 53.0, Align::Left),
            ],
            rows: report
                .bill
                .iter()
                .map(|r| {
                    vec![
                        Cell::text(r.name.clone()),
                        Cell::text(r.unit.clone()),
                        Cell::number(r.amount, if r.unit == "м" { 1 } else { 0 }),
                        Cell::text(r.note.clone()),
                    ]
                })
                .collect(),
            footer: None,
        })
    }

    /// Общие замечания: доступ к ревизии, нет стояка.
    pub(super) fn plumbing_notes(&self, water: bool) -> Option<Table> {
        let report = self.plumbing_report(water);
        (!report.warnings.is_empty()).then(|| Table {
            title: "Замечания".into(),
            columns: vec![Column::new("", 185.0, Align::Left)],
            rows: report
                .warnings
                .iter()
                .map(|w| vec![Cell::text(w.clone())])
                .collect(),
            footer: None,
        })
    }

    /// План водоснабжения или канализации.
    pub(super) fn draw_plumbing(&self, water: bool, plot: &mut Plot<'_>) {
        let (doc, derived) = self.final_view();
        let rooms = match &self.variant {
            Some(_) => self.variant_rooms(),
            None => self.survey_rooms(),
        };
        plot.walls(LAYER_WALLS, &walls_where(doc, derived, |_| true));
        plot.openings(doc, derived, |_| true);
        plot.room_labels(doc, &rooms, false);
        // Приборы — тонкими контурами.
        let layer = plot.sheet.layer(LAYER_FURNITURE);
        for f in doc
            .furniture
            .iter()
            .filter(|f| derived.visible.contains(&f.layer) && plumbing::is_fixture(f))
        {
            for m in crate::furniture::plan_marks(f, None) {
                plot.items.push(Item {
                    layer,
                    shape: Mark::Stroke {
                        points: m.points,
                        closed: m.closed,
                        width: THIN,
                        color: Rgb::hex(0x777777),
                        dash: None,
                    },
                });
            }
        }
        let layer = plot.sheet.layer(LAYER_PLUMBING);
        let k = plot.k;
        for p in doc
            .pipes
            .iter()
            .filter(|p| derived.visible.contains(&p.layer) && p.system.is_water() == water)
        {
            let (color, dash) = pen(p.system);
            let points: Vec<DVec2> = p.points.iter().map(plumbing::xy).collect();
            plot.items.push(Item {
                layer,
                shape: Mark::Stroke {
                    points: points.clone(),
                    closed: false,
                    width: 0.4,
                    color,
                    dash,
                },
            });
            // Обозначение системы у середины самого длинного участка.
            if let Some(w) = points
                .windows(2)
                .max_by(|a, b| a[0].distance(a[1]).total_cmp(&b[0].distance(b[1])))
                && w[0].distance(w[1]) > 8.0 * k
            {
                let mid = w[0].midpoint(w[1]);
                let up = if (w[1] - w[0]).x.abs() >= (w[1] - w[0]).y.abs() {
                    DVec2::new(0.0, -1.6 * k)
                } else {
                    DVec2::new(2.2 * k, 0.0)
                };
                plot.text(
                    layer,
                    mid + up,
                    plumbing::code(p.system),
                    SMALL_HEIGHT * 0.8,
                    Align::Center,
                    color,
                );
            }
        }
        for p in self.plumbing_points(water) {
            let (color, _) = pen(p.system);
            for m in plumbing::plan_marks(p) {
                plot.items.push(Item {
                    layer,
                    shape: Mark::Stroke {
                        points: m.points,
                        closed: m.closed,
                        width: SYMBOL_LINE,
                        color,
                        dash: None,
                    },
                });
            }
            let text = match p.kind {
                PlumbingKind::Outlet => format!("H{}", p.height.round()),
                PlumbingKind::Riser => format!("Ст {}", plumbing::code(p.system)),
                PlumbingKind::Manifold => format!("Кол. {}", plumbing::code(p.system)),
            };
            let at = plumbing::to_plan(p, DVec2::new(0.0, plumbing::SYMBOL * 1.6));
            plot.text(layer, at, &text, SMALL_HEIGHT * 0.7, Align::Center, color);
        }
    }
}
