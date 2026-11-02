//! Пример: записывает тестовый лист в PDF для проверки внешними программами.
//!
//! `cargo run -p op-export --example sample_sheets -- out.pdf`

use glam::DVec2;
use op_export::{
    Align, Item, Rgb, Sheet, Text, TitleBlock, Viewport, frame, hatch, pdf,
    table::{Cell, Column, Table},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "sample.pdf".into());
    let mut sheet = Sheet {
        title: "Обмерный план".into(),
        size: DVec2::new(420.0, 297.0),
        layers: vec![],
        paper: vec![],
        viewports: vec![],
    };
    let frame_layer = sheet.layer("Рамка и штамп");
    let walls = sheet.layer("Стены");
    let block = TitleBlock {
        designation: "2026-01-АР".into(),
        object: "Санкт-Петербург, Комендантский проспект, дом 1, кв. 25".into(),
        building: "Квартира · Вариант А".into(),
        sheet_title: "Обмерный план".into(),
        stage: "Р".into(),
        sheet: 1,
        sheets: 3,
        organization: "OpenPlanner".into(),
        developer: "Иванов".into(),
        checker: "Петров".into(),
        date: "11.26".into(),
    };
    let corner = frame::draw(sheet.size, &block, frame_layer, &mut sheet.paper);
    let table = Table {
        title: "Экспликация помещений".into(),
        columns: vec![
            Column::new("№", 15.0, Align::Center),
            Column::new("Наименование", 120.0, Align::Left),
            Column::new("Площадь, м²", 50.0, Align::Right),
        ],
        rows: vec![
            vec![Cell::text("1"), Cell::text("Кухня"), Cell::number(12.5, 2)],
            vec![
                Cell::text("2"),
                Cell::text("Спальня"),
                Cell::number(14.25, 2),
            ],
        ],
        footer: Some(vec![
            Cell::text(""),
            Cell::text("Итого"),
            Cell::number(26.75, 2),
        ]),
    };
    table.draw(
        frame_layer,
        corner - DVec2::new(0.0, table.height() + 5.0),
        &mut sheet.paper,
    );
    let ring = vec![
        DVec2::new(0.0, 0.0),
        DVec2::new(4000.0, 0.0),
        DVec2::new(4000.0, 3000.0),
        DVec2::new(0.0, 3000.0),
    ];
    let inner = vec![
        DVec2::new(120.0, 120.0),
        DVec2::new(3880.0, 120.0),
        DVec2::new(3880.0, 2880.0),
        DVec2::new(120.0, 2880.0),
    ];
    let mut items = vec![Item::fill(
        walls,
        vec![ring.clone(), inner.clone()],
        Rgb::hex(0xbbbbbb),
    )];
    for (a, b) in hatch::hatch(&[ring, inner], std::f64::consts::FRAC_PI_4, 100.0) {
        items.push(Item::line(walls, a, b, 0.18, Rgb::BLACK));
    }
    items.push(Item::text(
        walls,
        Text::centered(
            DVec2::new(2000.0, 1500.0),
            "Кухня 12,50 м²",
            3.5,
            Align::Center,
        ),
    ));
    sheet.viewports.push(Viewport {
        items,
        center: DVec2::new(2000.0, 1500.0),
        origin: DVec2::new(120.0, 120.0),
        denominator: 50.0,
        clip: (DVec2::new(20.0, 5.0), DVec2::new(225.0, 292.0)),
    });
    let bytes = pdf::write(
        &[sheet],
        &pdf::PdfInfo {
            title: "Пример".into(),
            author: "OpenPlanner".into(),
        },
    )?;
    std::fs::write(path, bytes)?;
    Ok(())
}
