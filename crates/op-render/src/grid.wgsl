// Фоновая сетка плана: полноэкранный треугольник, линии считаются во фрагментном шейдере.
// Толщина линий — 1 физический пиксель при любом масштабе.

struct Globals {
    viewport: vec2<f32>, // размер холста, физические пиксели
    center: vec2<f32>,   // точка плана в центре холста, мм
    scale: f32,          // физических пикселей на мм
    srgb_output: f32,    // 1 — поверхность sRGB, нужен перевод в линейное пространство
    _pad: vec2<f32>,
};

@group(0) @binding(0) var<uniform> g: Globals;

const BG: vec3<f32> = vec3<f32>(0.98431, 0.98431, 0.97255);    // #FBFBF8
const MINOR: vec3<f32> = vec3<f32>(0.93725, 0.93725, 0.91765); // #EFEFEA
const MAJOR: vec3<f32> = vec3<f32>(0.86275, 0.86275, 0.83922); // #DCDCD6

@vertex
fn vs_main(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let x = f32((i << 1u) & 2u) * 2.0 - 1.0;
    let y = f32(i & 2u) * 2.0 - 1.0;
    return vec4<f32>(x, y, 0.0, 1.0);
}

// Насколько точка близка к линии сетки с шагом `spacing` мм, с учётом видимости уровня.
fn grid_level(world: vec2<f32>, spacing: f32) -> f32 {
    let spacing_px = spacing * g.scale;
    let visibility = smoothstep(5.0, 14.0, spacing_px);
    let d = abs(fract(world / spacing + 0.5) - 0.5) * spacing_px;
    let line = 1.0 - smoothstep(0.0, 1.0, min(d.x, d.y));
    return line * visibility;
}

fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    let low = c / 12.92;
    let high = pow((c + 0.055) / 1.055, vec3<f32>(2.4));
    return select(high, low, c <= vec3<f32>(0.04045));
}

@fragment
fn fs_main(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let world = g.center + (pos.xy - g.viewport * 0.5) / g.scale;
    var color = BG;
    color = mix(color, MINOR, grid_level(world, 10.0));
    color = mix(color, MINOR, grid_level(world, 100.0));
    color = mix(color, MAJOR, grid_level(world, 1000.0));
    color = mix(color, MAJOR, grid_level(world, 10000.0));
    if (g.srgb_output > 0.5) {
        color = srgb_to_linear(color);
    }
    return vec4<f32>(color, 1.0);
}
