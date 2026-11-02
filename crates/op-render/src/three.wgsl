// 3D-вид: освещение небом и солнцем с тенью по карте глубины.
// Координаты — мм, Z вверх.

struct Globals3 {
    view_proj: mat4x4<f32>,
    sun_view_proj: mat4x4<f32>,
    sun: vec4<f32>,      // xyz — направление на солнце, w — сила прямого света (0 — нет солнца)
    params: vec4<f32>,   // x — 1, если поверхность sRGB; y — размер текселя карты теней
};

@group(0) @binding(0) var<uniform> g: Globals3;
@group(1) @binding(0) var shadow_map: texture_depth_2d;
@group(1) @binding(1) var shadow_sampler: sampler_comparison;

fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    let low = c / 12.92;
    let high = pow((c + 0.055) / 1.055, vec3<f32>(2.4));
    return select(high, low, c <= vec3<f32>(0.04045));
}

fn linear_to_srgb(c: vec3<f32>) -> vec3<f32> {
    let low = c * 12.92;
    let high = 1.055 * pow(c, vec3<f32>(1.0 / 2.4)) - 0.055;
    return select(high, low, c <= vec3<f32>(0.0031308));
}

struct VertexIn {
    @location(0) pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec4<f32>,
};

struct VertexOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) color: vec4<f32>,
    @location(2) shadow: vec3<f32>,
};

@vertex
fn mesh_vs(v: VertexIn) -> VertexOut {
    var out: VertexOut;
    out.clip = g.view_proj * vec4<f32>(v.pos, 1.0);
    out.normal = v.normal;
    out.color = v.color;
    let s = g.sun_view_proj * vec4<f32>(v.pos, 1.0);
    // Ортографическая проекция: w = 1. Текстурные координаты — Y вниз.
    out.shadow = vec3<f32>(s.x * 0.5 + 0.5, 0.5 - s.y * 0.5, s.z);
    return out;
}

// Доля прямого света: 4 выборки с сравнением (PCF).
fn lit(shadow: vec3<f32>, ndl: f32) -> f32 {
    if (shadow.x < 0.0 || shadow.x > 1.0 || shadow.y < 0.0 || shadow.y > 1.0 || shadow.z > 1.0) {
        return 1.0;
    }
    let bias = 0.0015 + 0.004 * (1.0 - ndl);
    let t = g.params.y;
    var sum = 0.0;
    for (var i = 0; i < 4; i++) {
        let o = vec2<f32>(f32(i % 2) - 0.5, f32(i / 2) - 0.5) * t;
        sum += textureSampleCompareLevel(shadow_map, shadow_sampler, shadow.xy + o, shadow.z - bias);
    }
    return sum * 0.25;
}

fn shade(v: VertexOut) -> vec4<f32> {
    let n = normalize(v.normal);
    let albedo = srgb_to_linear(v.color.rgb);
    // Небо сверху, отражённый свет снизу.
    let sky = mix(0.42, 0.78, n.z * 0.5 + 0.5);
    var light = vec3<f32>(sky);
    if (g.sun.w > 0.0) {
        let ndl = max(dot(n, g.sun.xyz), 0.0);
        let direct = g.sun.w * ndl * lit(v.shadow, ndl);
        light = light * 0.75 + vec3<f32>(1.0, 0.96, 0.88) * direct * 1.1;
    } else {
        // Без солнца — мягкий боковой свет, чтобы читались грани.
        let key = normalize(vec3<f32>(-0.4, -0.7, 0.6));
        light = light + vec3<f32>(0.22 * max(dot(n, key), 0.0));
    }
    var c = albedo * light;
    if (g.params.x < 0.5) {
        c = linear_to_srgb(c);
    }
    return vec4<f32>(c, v.color.a);
}

@fragment
fn mesh_fs(v: VertexOut) -> @location(0) vec4<f32> {
    return shade(v);
}

// ---------- Карта теней ----------

@vertex
fn shadow_vs(v: VertexIn) -> @builtin(position) vec4<f32> {
    return g.sun_view_proj * vec4<f32>(v.pos, 1.0);
}
