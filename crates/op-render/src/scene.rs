//! Сцена плана — то, что рисует рендерер: заливки, линии, текст, изображения.
//!
//! Сцена строится на CPU (в движке) и не зависит от GPU, поэтому её можно
//! проверять в тестах без видеокарты. Координаты — миллиметры плана.

use bytemuck::{Pod, Zeroable};
use glam::DVec2;
use lyon_tessellation::{
    BuffersBuilder, FillOptions, FillRule, FillTessellator, FillVertex, VertexBuffers, math::point,
    path::Path,
};

use crate::text::Font;

/// Цвет sRGB с прямой (не умноженной) альфой.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color(pub [f32; 4]);

impl Color {
    /// Цвет из `0xRRGGBB` и непрозрачности.
    #[must_use]
    pub fn hex(rgb: u32, alpha: f32) -> Self {
        let c = |shift: u32| f32::from(u8::try_from((rgb >> shift) & 0xFF).unwrap_or(0)) / 255.0;
        Self([c(16), c(8), c(0), alpha])
    }

    /// Тот же цвет с другой непрозрачностью.
    #[must_use]
    pub fn with_alpha(self, alpha: f32) -> Self {
        let [r, g, b, _] = self.0;
        Self([r, g, b, alpha])
    }
}

/// Вершина заливки.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable, PartialEq)]
pub struct FillVertexData {
    /// Точка плана, мм.
    pub pos: [f32; 2],
    /// Цвет.
    pub color: [f32; 4],
}

/// Отрезок или точка-маркер постоянной экранной толщины.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable, PartialEq)]
pub struct LineInstance {
    /// Начало, мм.
    pub a: [f32; 2],
    /// Конец, мм.
    pub b: [f32; 2],
    /// Толщина (для отрезка) или диаметр (для маркера), CSS-пиксели.
    pub width: f32,
    /// Длина штриха, CSS-пиксели; 0 — сплошная.
    pub dash: f32,
    /// 0 — отрезок, 1 — круг, 2 — окружность, 3 — квадрат с обводкой.
    pub kind: f32,
    pad: f32,
    /// Цвет.
    pub color: [f32; 4],
}

/// Глиф текста.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable, PartialEq)]
pub struct GlyphInstance {
    /// Точка привязки, мм.
    pub anchor: [f32; 2],
    /// Угол поворота, радианы (по часовой при Y вниз).
    pub angle: f32,
    /// Размер em: мм плана или CSS-пиксели (см. `screen`).
    pub size: f32,
    /// Левый верхний угол глифа относительно привязки, em.
    pub min: [f32; 2],
    /// Правый нижний угол, em.
    pub max: [f32; 2],
    /// Текстурные координаты.
    pub uv_min: [f32; 2],
    /// Текстурные координаты.
    pub uv_max: [f32; 2],
    /// 1 — размер в CSS-пикселях, 0 — в мм плана.
    pub screen: f32,
    /// Ширина белой обводки, CSS-пиксели.
    pub halo: f32,
    pad: [f32; 2],
    /// Цвет текста.
    pub color: [f32; 4],
}

/// Изображение на плане (подложка).
#[derive(Debug, Clone, PartialEq)]
pub struct ImageQuad {
    /// Ключ изображения, переданного в рендерер.
    pub key: String,
    /// Левый верхний угол, мм.
    pub origin: DVec2,
    /// Размер, мм.
    pub size: DVec2,
    /// Непрозрачность.
    pub opacity: f32,
}

/// Горизонтальное выравнивание текста.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Align {
    /// По левому краю.
    Left,
    /// По центру.
    #[default]
    Center,
    /// По правому краю.
    Right,
}

/// Оформление текста.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextStyle {
    /// Размер em.
    pub size: f64,
    /// Размер в CSS-пикселях (иначе — в мм плана, текст масштабируется с планом).
    pub screen: bool,
    /// Выравнивание по горизонтали.
    pub align: Align,
    /// Поворот, радианы.
    pub angle: f64,
    /// Цвет.
    pub color: Color,
    /// Белая обводка, CSS-пиксели.
    pub halo: f32,
}

/// Сцена.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Scene {
    /// Вершины заливок (треугольники по индексам).
    pub fill_vertices: Vec<FillVertexData>,
    /// Индексы треугольников заливок.
    pub fill_indices: Vec<u32>,
    /// Линии и маркеры.
    pub lines: Vec<LineInstance>,
    /// Глифы.
    pub glyphs: Vec<GlyphInstance>,
    /// Изображения (рисуются первыми).
    pub images: Vec<ImageQuad>,
}

#[allow(clippy::cast_possible_truncation)] // f32 достаточно для мм плана на GPU
fn f(p: DVec2) -> [f32; 2] {
    [p.x as f32, p.y as f32]
}

impl Scene {
    /// Пустая сцена.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Заливка фигуры с отверстиями (правило чёт-нечет).
    pub fn fill(&mut self, contours: &[&[DVec2]], color: Color) {
        let mut builder = Path::builder();
        let mut any = false;
        for contour in contours {
            if contour.len() < 3 {
                continue;
            }
            any = true;
            let [x, y] = f(contour[0]);
            builder.begin(point(x, y));
            for p in &contour[1..] {
                let [x, y] = f(*p);
                builder.line_to(point(x, y));
            }
            builder.close();
        }
        if !any {
            return;
        }
        let path = builder.build();
        let mut buffers: VertexBuffers<FillVertexData, u32> = VertexBuffers::new();
        let mut tess = FillTessellator::new();
        let result = tess.tessellate_path(
            &path,
            &FillOptions::tolerance(0.1).with_fill_rule(FillRule::EvenOdd),
            &mut BuffersBuilder::new(&mut buffers, |v: FillVertex| FillVertexData {
                pos: v.position().to_array(),
                color: color.0,
            }),
        );
        if result.is_err() {
            log::warn!("не удалось построить заливку");
            return;
        }
        let base = u32::try_from(self.fill_vertices.len()).unwrap_or(u32::MAX);
        self.fill_vertices.extend(buffers.vertices);
        self.fill_indices
            .extend(buffers.indices.into_iter().map(|i| i.saturating_add(base)));
    }

    /// Отрезок.
    pub fn line(&mut self, a: DVec2, b: DVec2, width: f32, color: Color) {
        self.push_line(a, b, width, 0.0, 0.0, color);
    }

    /// Штриховой отрезок.
    pub fn dashed(&mut self, a: DVec2, b: DVec2, width: f32, dash: f32, color: Color) {
        self.push_line(a, b, width, dash, 0.0, color);
    }

    /// Замкнутая или открытая ломаная.
    pub fn polyline(&mut self, points: &[DVec2], closed: bool, width: f32, color: Color) {
        for pair in points.windows(2) {
            self.line(pair[0], pair[1], width, color);
        }
        if closed && points.len() > 2 {
            self.line(points[points.len() - 1], points[0], width, color);
        }
    }

    /// Закрашенный круг диаметром `size` CSS-пикселей.
    pub fn dot(&mut self, p: DVec2, size: f32, color: Color) {
        self.push_line(p, p, size, 0.0, 1.0, color);
    }

    /// Окружность диаметром `size` CSS-пикселей.
    pub fn ring(&mut self, p: DVec2, size: f32, color: Color) {
        self.push_line(p, p, size, 0.0, 2.0, color);
    }

    /// Квадратная ручка со стороной `size` CSS-пикселей: белая с цветной обводкой.
    pub fn handle(&mut self, p: DVec2, size: f32, color: Color) {
        self.push_line(p, p, size, 0.0, 3.0, color);
    }

    fn push_line(&mut self, a: DVec2, b: DVec2, width: f32, dash: f32, kind: f32, color: Color) {
        self.lines.push(LineInstance {
            a: f(a),
            b: f(b),
            width,
            dash,
            kind,
            pad: 0.0,
            color: color.0,
        });
    }

    /// Строка текста с центром по высоте прописных букв в точке `at`.
    pub fn text(&mut self, font: &Font, at: DVec2, text: &str, style: TextStyle) {
        let line = font.layout(text);
        let dx = match style.align {
            Align::Left => 0.0,
            Align::Center => -line.width / 2.0,
            Align::Right => -line.width,
        };
        let dy = font.cap_height / 2.0;
        #[allow(clippy::cast_possible_truncation)] // f32 достаточно для GPU
        let (angle, size) = (style.angle as f32, style.size as f32);
        for placed in line.glyphs {
            let g = placed.glyph;
            self.glyphs.push(GlyphInstance {
                anchor: f(at),
                angle,
                size,
                min: [g.min[0] + placed.x + dx, g.min[1] + dy],
                max: [g.max[0] + placed.x + dx, g.max[1] + dy],
                uv_min: g.uv_min,
                uv_max: g.uv_max,
                screen: if style.screen { 1.0 } else { 0.0 },
                halo: style.halo,
                pad: [0.0; 2],
                color: style.color.0,
            });
        }
    }

    /// Ширина строки в em.
    #[must_use]
    pub fn text_width(font: &Font, text: &str) -> f64 {
        f64::from(font.layout(text).width)
    }

    /// Изображение.
    pub fn image(&mut self, quad: ImageQuad) {
        self.images.push(quad);
    }

    /// Добавляет другую сцену поверх этой.
    pub fn extend(&mut self, other: Self) {
        let base = u32::try_from(self.fill_vertices.len()).unwrap_or(u32::MAX);
        self.fill_vertices.extend(other.fill_vertices);
        self.fill_indices.extend(
            other
                .fill_indices
                .into_iter()
                .map(|i| i.saturating_add(base)),
        );
        self.lines.extend(other.lines);
        self.glyphs.extend(other.glyphs);
        self.images.extend(other.images);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fill_with_hole_is_tessellated() {
        let mut scene = Scene::new();
        let outer = [
            DVec2::ZERO,
            DVec2::new(100.0, 0.0),
            DVec2::new(100.0, 100.0),
            DVec2::new(0.0, 100.0),
        ];
        let hole = [
            DVec2::new(25.0, 25.0),
            DVec2::new(75.0, 25.0),
            DVec2::new(75.0, 75.0),
            DVec2::new(25.0, 75.0),
        ];
        scene.fill(&[&outer, &hole], Color::hex(0x000000, 1.0));
        let area: f32 = scene
            .fill_indices
            .as_chunks::<3>()
            .0
            .iter()
            .map(|[i, j, k]| {
                let p = |i: u32| scene.fill_vertices[i as usize].pos;
                let (a, b, c) = (p(*i), p(*j), p(*k));
                ((b[0] - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (b[1] - a[1])).abs() / 2.0
            })
            .sum();
        assert!((area - 7500.0).abs() < 1.0);
    }

    #[test]
    fn text_is_centered() {
        let font = Font::shared().unwrap();
        let mut scene = Scene::new();
        let style = TextStyle {
            size: 100.0,
            screen: false,
            align: Align::Center,
            angle: 0.0,
            color: Color::hex(0, 1.0),
            halo: 0.0,
        };
        scene.text(font, DVec2::ZERO, "ОБМЕР", style);
        let min = scene
            .glyphs
            .iter()
            .map(|g| g.min[0])
            .fold(f32::MAX, f32::min);
        let max = scene
            .glyphs
            .iter()
            .map(|g| g.max[0])
            .fold(f32::MIN, f32::max);
        assert!((min + max).abs() < 0.1, "{min} {max}");
    }

    #[test]
    fn hex_colors() {
        assert_eq!(Color::hex(0xFF8000, 0.5).0, [1.0, 128.0 / 255.0, 0.0, 0.5]);
    }
}
