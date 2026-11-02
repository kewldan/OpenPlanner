// Конвейеры плана: заливки, линии постоянной экранной толщины, SDF-текст, изображения.
// Мировые координаты — мм плана (Y вниз); экранные — физические пиксели.

struct Globals {
    viewport: vec2<f32>, // размер холста, физические пиксели
    center: vec2<f32>,   // точка плана в центре холста, мм
    scale: f32,          // физических пикселей на мм
    srgb_output: f32,    // 1 — поверхность sRGB: цвета переводятся в линейное пространство
    dpr: f32,            // физических пикселей в CSS-пикселе
    _pad: f32,
};

@group(0) @binding(0) var<uniform> g: Globals;

const ATLAS_EM: f32 = 48.0;
const SPREAD: f32 = 6.0;

fn world_to_px(p: vec2<f32>) -> vec2<f32> {
    return (p - g.center) * g.scale + g.viewport * 0.5;
}

fn px_to_clip(px: vec2<f32>) -> vec4<f32> {
    let n = px / g.viewport * 2.0 - 1.0;
    return vec4<f32>(n.x, -n.y, 0.0, 1.0);
}

fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    let low = c / 12.92;
    let high = pow((c + 0.055) / 1.055, vec3<f32>(2.4));
    return select(high, low, c <= vec3<f32>(0.04045));
}

fn out_color(c: vec4<f32>) -> vec4<f32> {
    if (g.srgb_output > 0.5) {
        return vec4<f32>(srgb_to_linear(c.rgb), c.a);
    }
    return c;
}

// Угол квадрата из двух треугольников: (0..1, 0..1).
fn corner(i: u32) -> vec2<f32> {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0),
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 1.0), vec2<f32>(0.0, 1.0),
    );
    return corners[i];
}

// ---------- Заливки ----------

struct FillOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn fill_vs(@location(0) pos: vec2<f32>, @location(1) color: vec4<f32>) -> FillOut {
    var out: FillOut;
    out.pos = px_to_clip(world_to_px(pos));
    out.color = color;
    return out;
}

@fragment
fn fill_fs(in: FillOut) -> @location(0) vec4<f32> {
    return out_color(in.color);
}

// ---------- Линии и маркеры ----------

struct LineOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) local: vec2<f32>,
    @location(1) params: vec4<f32>, // длина, полутолщина, штрих, вид
    @location(2) color: vec4<f32>,
};

@vertex
fn line_vs(
    @builtin(vertex_index) vi: u32,
    @location(0) a: vec2<f32>,
    @location(1) b: vec2<f32>,
    @location(2) width: f32,
    @location(3) dash: f32,
    @location(4) kind: f32,
    @location(5) color: vec4<f32>,
) -> LineOut {
    let c = corner(vi);
    let pa = world_to_px(a);
    let pb = world_to_px(b);
    let d = pb - pa;
    let len = length(d);
    var dir = vec2<f32>(1.0, 0.0);
    if (len > 0.0001) {
        dir = d / len;
    }
    let n = vec2<f32>(-dir.y, dir.x);
    let hw = width * g.dpr * 0.5;
    let ext = hw + 1.0;
    let along = mix(-ext, len + ext, c.x);
    let across = mix(-ext, ext, c.y);
    var out: LineOut;
    out.pos = px_to_clip(pa + dir * along + n * across);
    out.local = vec2<f32>(along, across);
    out.params = vec4<f32>(len, hw, dash * g.dpr, kind);
    out.color = color;
    return out;
}

@fragment
fn line_fs(in: LineOut) -> @location(0) vec4<f32> {
    let len = in.params.x;
    let hw = in.params.y;
    let dash = in.params.z;
    let kind = in.params.w;
    var color = in.color;
    var cov = 1.0;
    if (kind < 0.5) {
        let beyond = max(0.0, max(-in.local.x, in.local.x - len));
        cov = clamp(hw + 0.5 - abs(in.local.y), 0.0, 1.0) * clamp(hw + 0.5 - beyond, 0.0, 1.0);
        if (dash > 0.0 && fract(in.local.x / (dash * 2.0)) > 0.5) {
            cov = 0.0;
        }
    } else if (kind < 1.5) {
        cov = clamp(hw + 0.5 - length(in.local), 0.0, 1.0);
    } else if (kind < 2.5) {
        let stroke = 0.75 * g.dpr;
        cov = clamp(stroke + 0.5 - abs(length(in.local) - hw + stroke), 0.0, 1.0);
    } else {
        let m = max(abs(in.local.x), abs(in.local.y));
        cov = clamp(hw + 0.5 - m, 0.0, 1.0);
        let border = clamp(m - (hw - 1.5 * g.dpr) + 0.5, 0.0, 1.0);
        color = vec4<f32>(mix(vec3<f32>(1.0), color.rgb, border), color.a);
    }
    return out_color(vec4<f32>(color.rgb, color.a * cov));
}

// ---------- Текст ----------

@group(1) @binding(0) var tex: texture_2d<f32>;
@group(1) @binding(1) var samp: sampler;

struct GlyphOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) params: vec2<f32>, // пикселей на тексель атласа, обводка
    @location(2) color: vec4<f32>,
};

@vertex
fn glyph_vs(
    @builtin(vertex_index) vi: u32,
    @location(0) anchor: vec2<f32>,
    @location(1) angle: f32,
    @location(2) size: f32,
    @location(3) gmin: vec2<f32>,
    @location(4) gmax: vec2<f32>,
    @location(5) uv_min: vec2<f32>,
    @location(6) uv_max: vec2<f32>,
    @location(7) screen: f32,
    @location(8) halo: f32,
    @location(9) color: vec4<f32>,
) -> GlyphOut {
    let c = corner(vi);
    var em_px = size * g.scale;
    if (screen > 0.5) {
        em_px = size * g.dpr;
    }
    var local = mix(gmin, gmax, c) * em_px;
    // Слишком мелкий текст не рисуется: квадрат схлопывается.
    if (em_px < 3.0 * g.dpr) {
        local = vec2<f32>(0.0);
    }
    let cs = cos(angle);
    let sn = sin(angle);
    let rotated = vec2<f32>(local.x * cs - local.y * sn, local.x * sn + local.y * cs);
    var out: GlyphOut;
    out.pos = px_to_clip(world_to_px(anchor) + rotated);
    out.uv = mix(uv_min, uv_max, c);
    out.params = vec2<f32>(em_px / ATLAS_EM, halo * g.dpr);
    out.color = color;
    return out;
}

// Расстояние до контура в пикселях экрана (положительно внутри глифа).
fn glyph_distance(uv: vec2<f32>, px_per_texel: f32) -> f32 {
    let d = textureSample(tex, samp, uv).r;
    return (d - 0.5) * 2.0 * SPREAD * px_per_texel;
}

// Покрытие и среднее расстояние по четырём точкам внутри пикселя: атлас при мелком
// тексте сильно уменьшен, и без усреднения тонкие штрихи пропадают.
fn glyph_coverage(in: GlyphOut) -> vec2<f32> {
    let dx = dpdx(in.uv) * 0.25;
    let dy = dpdy(in.uv) * 0.25;
    let k = in.params.x;
    let d0 = glyph_distance(in.uv - dx - dy, k);
    let d1 = glyph_distance(in.uv + dx - dy, k);
    let d2 = glyph_distance(in.uv - dx + dy, k);
    let d3 = glyph_distance(in.uv + dx + dy, k);
    let cov = (clamp(d0 + 0.5, 0.0, 1.0) + clamp(d1 + 0.5, 0.0, 1.0)
        + clamp(d2 + 0.5, 0.0, 1.0) + clamp(d3 + 0.5, 0.0, 1.0)) * 0.25;
    return vec2<f32>(cov, (d0 + d1 + d2 + d3) * 0.25);
}

// Первый проход: светлая обводка под всеми буквами (иначе обводка следующей
// буквы закрывала бы край предыдущей).
@fragment
fn glyph_halo_fs(in: GlyphOut) -> @location(0) vec4<f32> {
    let c = glyph_coverage(in);
    var a = 0.0;
    if (in.params.y > 0.0) {
        a = clamp(c.y + in.params.y + 0.5, 0.0, 1.0) * 0.92;
    }
    if (a <= 0.0) {
        discard;
    }
    return out_color(vec4<f32>(0.984, 0.984, 0.973, a));
}

// Второй проход: сами буквы.
@fragment
fn glyph_fs(in: GlyphOut) -> @location(0) vec4<f32> {
    let a = glyph_coverage(in).x * in.color.a;
    if (a <= 0.0) {
        discard;
    }
    return out_color(vec4<f32>(in.color.rgb, a));
}

// ---------- Изображения ----------

struct ImageOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) opacity: f32,
};

@vertex
fn image_vs(@location(0) pos: vec2<f32>, @location(1) uv: vec2<f32>, @location(2) opacity: f32) -> ImageOut {
    var out: ImageOut;
    out.pos = px_to_clip(world_to_px(pos));
    out.uv = uv;
    out.opacity = opacity;
    return out;
}

@fragment
fn image_fs(in: ImageOut) -> @location(0) vec4<f32> {
    let c = textureSample(tex, samp, in.uv);
    return out_color(vec4<f32>(c.rgb, c.a * in.opacity));
}
