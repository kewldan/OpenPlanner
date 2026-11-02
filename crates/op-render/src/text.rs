//! Текст на холсте: SDF-атлас глифов и раскладка строк.
//!
//! Шрифт Onest (SIL OFL 1.1, `assets/Onest-OFL.txt`) встроен в движок. Глифы
//! растеризуются один раз при первом обращении; по растру строится поле
//! расстояний (SDF), поэтому текст остаётся чётким при любом масштабе.

use std::{collections::HashMap, sync::OnceLock};

use ab_glyph::{Font as _, FontRef, PxScale, ScaleFont as _};

/// Размер em при растеризации, пиксели атласа.
pub const ATLAS_EM: f32 = 48.0;
/// Сколько пикселей расстояния кодирует атлас по каждую сторону контура.
pub const SPREAD: f32 = 6.0;
/// Ширина атласа, пиксели.
const ATLAS_WIDTH: u32 = 1024;

const FONT_BYTES: &[u8] = include_bytes!("../assets/Onest-Medium.ttf");

/// Символы, которые попадают в атлас.
fn charset() -> impl Iterator<Item = char> {
    (' '..='~')
        .chain('А'..='я')
        .chain("ЁёІіЇїЄєЎў№²³°×−–—«»…·±⌀€₽≈≤≥←→↑↓".chars())
}

/// Глиф в атласе.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Glyph {
    /// Левый верхний угол квадрата глифа относительно точки на базовой линии, em (Y вниз).
    pub min: [f32; 2],
    /// Правый нижний угол, em.
    pub max: [f32; 2],
    /// Текстурные координаты левого верхнего угла (0–1).
    pub uv_min: [f32; 2],
    /// Текстурные координаты правого нижнего угла.
    pub uv_max: [f32; 2],
}

/// Шрифт с атласом расстояний.
#[derive(Debug)]
pub struct Font {
    glyphs: HashMap<char, (Option<Glyph>, f32, ab_glyph::GlyphId)>,
    font: FontRef<'static>,
    /// Пиксели атласа (R8): 0,5 — контур, больше — внутри.
    pub atlas: Vec<u8>,
    /// Ширина атласа.
    pub atlas_width: u32,
    /// Высота атласа.
    pub atlas_height: u32,
    /// Подъём над базовой линией, em.
    pub ascent: f32,
    /// Высота прописных букв (приблизительно), em.
    pub cap_height: f32,
}

/// Позиционированный глиф строки.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlacedGlyph {
    /// Глиф.
    pub glyph: Glyph,
    /// Сдвиг глифа по X от начала строки, em.
    pub x: f32,
}

/// Раскладка строки.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Line {
    /// Глифы.
    pub glyphs: Vec<PlacedGlyph>,
    /// Ширина строки, em.
    pub width: f32,
}

impl Font {
    /// Общий экземпляр встроенного шрифта (строится при первом вызове).
    #[must_use]
    pub fn shared() -> Option<&'static Self> {
        static FONT: OnceLock<Option<Font>> = OnceLock::new();
        FONT.get_or_init(Self::build).as_ref()
    }

    fn build() -> Option<Self> {
        let font = FontRef::try_from_slice(FONT_BYTES).ok()?;
        let scale = PxScale::from(ATLAS_EM);
        let scaled = font.as_scaled(scale);
        let pad = SPREAD.ceil() + 1.0;

        // Раскладка ячеек по строкам атласа.
        let mut cells = Vec::new();
        let (mut x, mut y, mut row_h) = (0u32, 0u32, 0u32);
        let mut glyphs = HashMap::new();
        for ch in charset() {
            let id = font.glyph_id(ch);
            if id.0 == 0 && ch != ' ' {
                continue;
            }
            let advance = scaled.h_advance(id) / ATLAS_EM;
            let outlined = font.outline_glyph(id.with_scale(scale));
            let Some(outlined) = outlined else {
                glyphs.insert(ch, (None, advance, id));
                continue;
            };
            let bounds = outlined.px_bounds();
            let w = to_px(bounds.width() + 2.0 * pad);
            let h = to_px(bounds.height() + 2.0 * pad);
            if x + w > ATLAS_WIDTH {
                x = 0;
                y += row_h + 1;
                row_h = 0;
            }
            cells.push((ch, outlined, x, y, w, h, bounds, advance, id));
            x += w + 1;
            row_h = row_h.max(h);
        }
        let atlas_height = (y + row_h + 1).next_power_of_two();
        let mut atlas = vec![0u8; (ATLAS_WIDTH * atlas_height) as usize];

        for (ch, outlined, cx, cy, w, h, bounds, advance, id) in cells {
            let mut coverage = vec![0f32; (w * h) as usize];
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            // pad — малое положительное число
            let off = pad as u32;
            outlined.draw(|gx, gy, c| {
                let (px, py) = (gx + off, gy + off);
                if px < w && py < h {
                    coverage[(py * w + px) as usize] = c;
                }
            });
            let field = signed_distance(&coverage, w as usize, h as usize);
            for row in 0..h {
                for col in 0..w {
                    let d = field[(row * w + col) as usize];
                    let v = (0.5 + d / (2.0 * SPREAD)).clamp(0.0, 1.0);
                    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // v ∈ [0, 1]
                    let byte = (v * 255.0).round() as u8;
                    atlas[((cy + row) * ATLAS_WIDTH + cx + col) as usize] = byte;
                }
            }
            #[allow(clippy::cast_precision_loss)] // размеры атласа малы
            let glyph = Glyph {
                min: [
                    (bounds.min.x - pad) / ATLAS_EM,
                    (bounds.min.y - pad) / ATLAS_EM,
                ],
                max: [
                    (bounds.min.x - pad + w as f32) / ATLAS_EM,
                    (bounds.min.y - pad + h as f32) / ATLAS_EM,
                ],
                uv_min: [
                    cx as f32 / ATLAS_WIDTH as f32,
                    cy as f32 / atlas_height as f32,
                ],
                uv_max: [
                    (cx + w) as f32 / ATLAS_WIDTH as f32,
                    (cy + h) as f32 / atlas_height as f32,
                ],
            };
            glyphs.insert(ch, (Some(glyph), advance, id));
        }

        let cap_height = font
            .outline_glyph(font.glyph_id('H').with_scale(scale))
            .map_or(0.7, |g| -g.px_bounds().min.y / ATLAS_EM);
        Some(Self {
            glyphs,
            ascent: scaled.ascent() / ATLAS_EM,
            cap_height,
            font,
            atlas,
            atlas_width: ATLAS_WIDTH,
            atlas_height,
        })
    }

    /// Раскладывает строку в одну линию. Неизвестные символы заменяются на «?».
    #[must_use]
    pub fn layout(&self, text: &str) -> Line {
        let scaled = self.font.as_scaled(PxScale::from(ATLAS_EM));
        let mut line = Line::default();
        let mut pen = 0.0f32;
        let mut previous = None;
        for ch in text.chars() {
            let Some(&(glyph, advance, id)) =
                self.glyphs.get(&ch).or_else(|| self.glyphs.get(&'?'))
            else {
                continue;
            };
            if let Some(prev) = previous {
                pen += scaled.kern(prev, id) / ATLAS_EM;
            }
            if let Some(glyph) = glyph {
                line.glyphs.push(PlacedGlyph { glyph, x: pen });
            }
            pen += advance;
            previous = Some(id);
        }
        line.width = pen;
        line
    }
}

fn to_px(v: f32) -> u32 {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    // размеры глифа малы и положительны
    let px = v.ceil().max(1.0) as u32;
    px
}

/// Поле расстояний: положительно внутри контура, в пикселях.
fn signed_distance(coverage: &[f32], w: usize, h: usize) -> Vec<f32> {
    let inside: Vec<bool> = coverage.iter().map(|&c| c >= 0.5).collect();
    let to_outside = edt(&inside.iter().map(|&i| !i).collect::<Vec<_>>(), w, h);
    let to_inside = edt(&inside, w, h);
    inside
        .iter()
        .enumerate()
        .map(|(i, &is_in)| {
            if is_in {
                to_outside[i].sqrt() - 0.5
            } else {
                -(to_inside[i].sqrt() - 0.5)
            }
        })
        .collect()
}

/// Квадрат евклидова расстояния до ближайшего пикселя `feature` (Felzenszwalb, Huttenlocher).
fn edt(feature: &[bool], w: usize, h: usize) -> Vec<f32> {
    const INF: f32 = 1e20;
    let mut grid: Vec<f32> = feature.iter().map(|&f| if f { 0.0 } else { INF }).collect();
    let n = w.max(h);
    let mut f = vec![0f32; n];
    let mut d = vec![0f32; n];
    let mut v = vec![0usize; n];
    let mut z = vec![0f32; n + 1];
    for x in 0..w {
        for y in 0..h {
            f[y] = grid[y * w + x];
        }
        edt_1d(&f[..h], &mut d[..h], &mut v, &mut z);
        for y in 0..h {
            grid[y * w + x] = d[y];
        }
    }
    for y in 0..h {
        f[..w].copy_from_slice(&grid[y * w..(y + 1) * w]);
        edt_1d(&f[..w], &mut d[..w], &mut v, &mut z);
        grid[y * w..(y + 1) * w].copy_from_slice(&d[..w]);
    }
    grid
}

#[allow(clippy::cast_precision_loss)] // индексы меньше размера глифа
fn edt_1d(f: &[f32], d: &mut [f32], v: &mut [usize], z: &mut [f32]) {
    let n = f.len();
    if n == 0 {
        return;
    }
    let mut k = 0usize;
    v[0] = 0;
    z[0] = f32::NEG_INFINITY;
    z[1] = f32::INFINITY;
    for q in 1..n {
        let fq = f[q] + (q * q) as f32;
        let mut s;
        loop {
            let p = v[k];
            s = (fq - (f[p] + (p * p) as f32)) / (2.0 * q as f32 - 2.0 * p as f32);
            if s <= z[k] && k > 0 {
                k -= 1;
            } else {
                break;
            }
        }
        if s <= z[k] {
            // k == 0 и парабола q целиком ниже — заменяет первую.
            v[0] = q;
            z[0] = f32::NEG_INFINITY;
            z[1] = f32::INFINITY;
            continue;
        }
        k += 1;
        v[k] = q;
        z[k] = s;
        z[k + 1] = f32::INFINITY;
    }
    k = 0;
    for (q, out) in d.iter_mut().enumerate().take(n) {
        while z[k + 1] < q as f32 {
            k += 1;
        }
        let p = v[k];
        let diff = q as f32 - p as f32;
        *out = diff * diff + f[p];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn font_has_latin_cyrillic_and_digits() {
        let font = Font::shared().unwrap();
        for text in ["Кухня 12,5 м²", "W 3000", "№ 1"] {
            let line = font.layout(text);
            assert!(line.width > 1.0, "{text}");
            assert_eq!(
                line.glyphs.len(),
                text.chars().filter(|c| !c.is_whitespace()).count(),
                "{text}"
            );
        }
        assert!(font.cap_height > 0.5 && font.cap_height < 0.9);
    }

    #[test]
    fn atlas_marks_glyph_interior() {
        let font = Font::shared().unwrap();
        let line = font.layout("I");
        let g = line.glyphs[0].glyph;
        // Середина ячейки «I» — внутри штриха.
        let u = f32::midpoint(g.uv_min[0], g.uv_max[0]);
        let v = f32::midpoint(g.uv_min[1], g.uv_max[1]);
        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            clippy::cast_precision_loss
        )]
        let (x, y) = (
            (u * font.atlas_width as f32) as usize,
            (v * font.atlas_height as f32) as usize,
        );
        assert!(font.atlas[y * font.atlas_width as usize + x] > 140);
        // Угол ячейки — снаружи.
        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            clippy::cast_precision_loss
        )]
        let (x0, y0) = (
            (g.uv_min[0] * font.atlas_width as f32) as usize,
            (g.uv_min[1] * font.atlas_height as f32) as usize,
        );
        assert!(font.atlas[y0 * font.atlas_width as usize + x0] < 30);
    }

    #[test]
    fn distance_transform_is_euclidean() {
        let (w, h) = (7, 7);
        let mut feature = vec![false; w * h];
        feature[3 * w + 3] = true;
        let d = edt(&feature, w, h);
        assert!((d[3 * w + 3]).abs() < 1e-6);
        assert!((d[3 * w + 6] - 9.0).abs() < 1e-6);
        assert!((d[0] - 18.0).abs() < 1e-6);
    }
}
