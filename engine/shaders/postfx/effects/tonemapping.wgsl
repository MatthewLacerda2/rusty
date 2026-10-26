// engine/shaders/postfx/effects/tonemapping.wgsl — Tonemapping (#1440): the operator
// that brings scene-linear HDR to the display (slot: x its index, 0 none, 1 Reinhard,
// 2 ACES), and, last in the grade, the display-gamma tweak (slot: x the gamma).

#define_import_path postfx::effects::tonemapping

fn aces(x: vec3<f32>) -> vec3<f32> {
    let a = 2.51; let b = 0.03; let c = 2.43; let d = 0.59; let e = 0.14;
    return clamp((x * (a * x + b)) / (x * (c * x + d) + e), vec3<f32>(0.0), vec3<f32>(1.0));
}

fn tonemap_with(c: vec3<f32>, mode: u32) -> vec3<f32> {
    if (mode == 1u) {
        return c / (c + vec3<f32>(1.0));
    } else if (mode == 2u) {
        return aces(c);
    }
    return clamp(c, vec3<f32>(0.0), vec3<f32>(1.0));
}

fn tonemap(c: vec3<f32>, p: vec4<f32>) -> vec3<f32> {
    return tonemap_with(c, u32(p.x));
}

// The display-gamma tweak (neutral 1), NOT the output encode: every target the
// composite writes is sRGB, and the hardware encodes on write (#415).
fn display_gamma(c_in: vec3<f32>, p: vec4<f32>) -> vec3<f32> {
    let c = clamp(c_in, vec3<f32>(0.0), vec3<f32>(1.0));
    return pow(c, vec3<f32>(1.0 / max(p.x, 0.01)));
}
