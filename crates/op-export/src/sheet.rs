//! Векторное описание листа.

use glam::DVec2;

/// Цвет без прозрачности.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    /// Чёрный — основной цвет чертежа.
    pub const BLACK: Self = Self(0, 0, 0);
    /// Белый.
    pub const WHITE: Self = Self(255, 255, 255);

    /// Цвет из `0xRRGGBB`.
    #[must_use]
    pub const fn hex(value: u32) -> Self {
        Self(
            ((value >> 16) & 0xff) as u8,
            ((value >> 8) & 0xff) as u8,
            (value & 0xff) as u8,
        )
    }

    /// Компоненты в диапазоне 0–1.
    #[must_use]
    pub fn unit(self) -> [f64; 3] {
        [self.0, self.1, self.2].map(|c| f64::from(c) / 255.0)
    }
}

/// Выравнивание текста относительно точки.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    /// Точка — начало строки.
    Left,
    /// Точка — середина строки.
    Center,
    /// Точка — конец строки.
    Right,
}

/// Строка текста.
#[derive(Debug, Clone, PartialEq)]
pub struct Text {
    /// Точка на базовой линии, мм.
    pub at: DVec2,
    /// Текст.
    pub text: String,
    /// Высота прописных букв, мм бумаги (размер шрифта по ГОСТ 2.304).
    pub height: f64,
    /// Выравнивание по горизонтали.
    pub align: Align,
    /// Поворот, радианы; ось Y направлена вниз, поэтому положительный угол — по часовой стрелке.
    pub angle: f64,
    /// Цвет.
    pub color: Rgb,
}

impl Text {
    /// Горизонтальная строка, центр которой по высоте прописных — в точке `at`.
    #[must_use]
    pub fn centered(at: DVec2, text: impl Into<String>, height: f64, align: Align) -> Self {
        Self {
            at: at + DVec2::new(0.0, height / 2.0),
            text: text.into(),
            height,
            align,
            angle: 0.0,
            color: Rgb::BLACK,
        }
    }
}

/// Примитив.
#[derive(Debug, Clone, PartialEq)]
pub enum Shape {
    /// Заливка контуров по правилу чёт-нечет.
    Fill {
        /// Замкнутые контуры.
        rings: Vec<Vec<DVec2>>,
        /// Цвет.
        color: Rgb,
    },
    /// Линия или ломаная.
    Stroke {
        /// Точки.
        points: Vec<DVec2>,
        /// Замкнутая.
        closed: bool,
        /// Толщина, мм бумаги.
        width: f64,
        /// Цвет.
        color: Rgb,
        /// Штрих и промежуток, мм бумаги; без значения — сплошная.
        dash: Option<(f64, f64)>,
    },
    /// Текст.
    Text(Text),
}

/// Примитив на слое.
#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    /// Номер слоя в [`Sheet::layers`].
    pub layer: usize,
    /// Примитив.
    pub shape: Shape,
}

impl Item {
    /// Отрезок.
    #[must_use]
    pub fn line(layer: usize, a: DVec2, b: DVec2, width: f64, color: Rgb) -> Self {
        Self::polyline(layer, vec![a, b], false, width, color)
    }

    /// Ломаная.
    #[must_use]
    pub fn polyline(
        layer: usize,
        points: Vec<DVec2>,
        closed: bool,
        width: f64,
        color: Rgb,
    ) -> Self {
        Self {
            layer,
            shape: Shape::Stroke {
                points,
                closed,
                width,
                color,
                dash: None,
            },
        }
    }

    /// Прямоугольник по углам.
    #[must_use]
    pub fn rect(layer: usize, lo: DVec2, hi: DVec2, width: f64) -> Self {
        Self::polyline(layer, rect_points(lo, hi), true, width, Rgb::BLACK)
    }

    /// Заливка.
    #[must_use]
    pub fn fill(layer: usize, rings: Vec<Vec<DVec2>>, color: Rgb) -> Self {
        Self {
            layer,
            shape: Shape::Fill { rings, color },
        }
    }

    /// Текст.
    #[must_use]
    pub fn text(layer: usize, text: Text) -> Self {
        Self {
            layer,
            shape: Shape::Text(text),
        }
    }
}

/// Углы прямоугольника по порядку обхода.
#[must_use]
pub fn rect_points(lo: DVec2, hi: DVec2) -> Vec<DVec2> {
    vec![lo, DVec2::new(hi.x, lo.y), hi, DVec2::new(lo.x, hi.y)]
}

/// Видовой экран: план в масштабе внутри прямоугольника листа.
///
/// Координаты примитивов — миллиметры плана; толщины, штрихи и высота текста — миллиметры бумаги.
#[derive(Debug, Clone, PartialEq)]
pub struct Viewport {
    /// Примитивы плана.
    pub items: Vec<Item>,
    /// Точка плана, которая попадает в `origin`.
    pub center: DVec2,
    /// Точка листа, мм.
    pub origin: DVec2,
    /// Знаменатель масштаба (50 для 1:50).
    pub denominator: f64,
    /// Обрезка: прямоугольник листа (мин, макс).
    pub clip: (DVec2, DVec2),
}

impl Viewport {
    /// Точка плана → точка листа.
    #[must_use]
    pub fn to_paper(&self, p: DVec2) -> DVec2 {
        self.origin + (p - self.center) / self.denominator
    }
}

/// Лист чертежа.
#[derive(Debug, Clone, PartialEq)]
pub struct Sheet {
    /// Название листа («Обмерный план»).
    pub title: String,
    /// Размер листа, мм (ширина, высота).
    pub size: DVec2,
    /// Слои; для PDF — слои документа, для DXF — слои чертежа.
    pub layers: Vec<String>,
    /// Примитивы в координатах листа.
    pub paper: Vec<Item>,
    /// Видовые экраны с планом.
    pub viewports: Vec<Viewport>,
}

/// Группа примитивов в координатах листа: с обрезкой (видовой экран) или без.
#[derive(Debug, Clone, PartialEq)]
pub struct Group {
    /// Обрезка.
    pub clip: Option<(DVec2, DVec2)>,
    /// Примитивы.
    pub items: Vec<Item>,
}

impl Sheet {
    /// Номер слоя по названию; новый слой добавляется в конец.
    pub fn layer(&mut self, name: &str) -> usize {
        if let Some(i) = self.layers.iter().position(|l| l == name) {
            return i;
        }
        self.layers.push(name.to_owned());
        self.layers.len() - 1
    }

    /// Все примитивы в координатах листа: сначала видовые экраны, затем рамка и таблицы.
    #[must_use]
    pub fn groups(&self) -> Vec<Group> {
        let mut out: Vec<Group> = self
            .viewports
            .iter()
            .map(|v| Group {
                clip: Some(v.clip),
                items: v
                    .items
                    .iter()
                    .map(|item| Item {
                        layer: item.layer,
                        shape: match &item.shape {
                            Shape::Fill { rings, color } => Shape::Fill {
                                rings: rings
                                    .iter()
                                    .map(|r| r.iter().map(|p| v.to_paper(*p)).collect())
                                    .collect(),
                                color: *color,
                            },
                            Shape::Stroke {
                                points,
                                closed,
                                width,
                                color,
                                dash,
                            } => Shape::Stroke {
                                points: points.iter().map(|p| v.to_paper(*p)).collect(),
                                closed: *closed,
                                width: *width,
                                color: *color,
                                dash: *dash,
                            },
                            Shape::Text(t) => Shape::Text(Text {
                                at: v.to_paper(t.at),
                                ..t.clone()
                            }),
                        },
                    })
                    .collect(),
            })
            .collect();
        out.push(Group {
            clip: None,
            items: self.paper.clone(),
        });
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn viewport_maps_plan_to_paper() {
        let v = Viewport {
            items: vec![],
            center: DVec2::new(1000.0, 1000.0),
            origin: DVec2::new(100.0, 100.0),
            denominator: 50.0,
            clip: (DVec2::ZERO, DVec2::splat(200.0)),
        };
        let p = v.to_paper(DVec2::new(3500.0, 0.0));
        assert!((p - DVec2::new(150.0, 80.0)).length() < 1e-9);
    }

    #[test]
    fn hex_colors() {
        assert_eq!(Rgb::hex(0x10_20_30), Rgb(0x10, 0x20, 0x30));
    }
}
