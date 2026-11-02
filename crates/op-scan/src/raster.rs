//! Вид сверху для подложки: пол — светлым, стены — тёмным, остальное прозрачно.

use crate::{ScanRaster, detect::Levels, samples::Sample};

/// Наибольшая сторона изображения, пиксели.
const MAX_SIDE: f64 = 4096.0;

/// Рисует вид сверху по выборке.
#[must_use]
pub fn render(samples: &[Sample], levels: Option<&Levels>) -> Option<ScanRaster> {
    let (lo, hi) = samples.iter().fold(
        (glam::DVec2::splat(f64::MAX), glam::DVec2::splat(f64::MIN)),
        |(lo, hi), s| (lo.min(s.p.truncate()), hi.max(s.p.truncate())),
    );
    if samples.is_empty() || hi.x <= lo.x || hi.y <= lo.y {
        return None;
    }
    let margin = 200.0;
    let (lo, hi) = (lo - margin, hi + margin);
    let size = hi - lo;
    let mm_per_px = (size.max_element() / MAX_SIDE).max(20.0);
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // ≤ MAX_SIDE
    let (w, h) = (
        ((size.x / mm_per_px).ceil() as usize).max(1),
        ((size.y / mm_per_px).ceil() as usize).max(1),
    );
    let mut floor = vec![0.0f64; w * h];
    let mut walls = vec![0.0f64; w * h];
    let (z0, z1) = levels.map_or((f64::MIN, f64::MAX), |l| (l.floor + 300.0, l.top() - 300.0));
    let floor_z = levels.map(|l| l.floor);
    for s in samples {
        let q = (s.p.truncate() - lo) / mm_per_px;
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // внутри изображения
        let (x, y) = (
            (q.x.max(0.0) as usize).min(w - 1),
            (q.y.max(0.0) as usize).min(h - 1),
        );
        let i = y * w + x;
        if s.n.z.abs() < 0.3 && (z0..=z1).contains(&s.p.z) {
            walls[i] += s.w;
        } else if s.n.z.abs() > 0.9 && floor_z.is_none_or(|f| (s.p.z - f).abs() < 80.0) {
            floor[i] += s.w;
        }
    }
    let pixel_area = mm_per_px * mm_per_px;
    let mut rgba = Vec::with_capacity(w * h * 4);
    for i in 0..w * h {
        // Плотность стены: площадь вертикальной поверхности над пикселем.
        let wall = (walls[i] / (pixel_area * 20.0)).min(1.0);
        if wall > 0.05 {
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // 0–255
            let v = (150.0 - 110.0 * wall) as u8;
            rgba.extend_from_slice(&[v, v, v, 255]);
        } else if floor[i] > 0.0 {
            rgba.extend_from_slice(&[226, 221, 206, 255]);
        } else {
            rgba.extend_from_slice(&[0, 0, 0, 0]);
        }
    }
    let png = encode(&rgba, w, h)?;
    Some(ScanRaster {
        png,
        x: lo.x,
        y: lo.y,
        width_px: u32::try_from(w).ok()?,
        height_px: u32::try_from(h).ok()?,
        mm_per_px,
    })
}

fn encode(rgba: &[u8], w: usize, h: usize) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    {
        let mut encoder =
            png::Encoder::new(&mut out, u32::try_from(w).ok()?, u32::try_from(h).ok()?);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().ok()?;
        writer.write_image_data(rgba).ok()?;
    }
    Some(out)
}
