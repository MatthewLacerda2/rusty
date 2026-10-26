// engine/shaders/postfx/effects/vignette.wgsl — the Vignette (#1439), Unity URP's,
// unrounded, on the graded image. Slot 0: x intensity (`0`: off), y smoothness, zw the
// centre uv; slot 1: rgb the colour the edges fade to.

#define_import_path postfx::effects::vignette

#import postfx::frame::Frag

fn vignette(c: vec3<f32>, uv: vec2<f32>, p: vec4<f32>, color: vec4<f32>) -> vec3<f32> {
    let d = abs(uv - p.zw) * p.x * 3.0;
    let falloff = pow(saturate(1.0 - dot(d, d)), p.y * 5.0);
    return c * mix(color.rgb, vec3<f32>(1.0), falloff);
}

fn display(c: vec3<f32>, f: Frag, p: vec4<f32>, color: vec4<f32>) -> vec3<f32> {
    var out = c;
    if (p.x > 0.0) {
        out = vignette(out, f.uv, p, color);
    }
    return out;
}
