// engine/shaders/postfx/rcas.wgsl — contrast-adaptive sharpening (#1847): AMD's
// FidelityFX RCAS (Robust Contrast-Adaptive Sharpening, MIT), the sharpen URP's TAA
// *Contrast Adaptive Sharpening* and FSR run in its final post pass, after
// tonemapping. One pass over the chain's LDR image at the chain's size
// (`PostFx::chain_size`), so bloom and the exposure meter read the unsharpened scene.
//
// RCAS reads the 5-tap cross around each pixel, finds the sharpening lobe the local
// contrast allows, and adds it, so an edge's contrast rises without ringing on a
// flat area (the lobe is clamped by the neighbourhood's own range). `sharp` is the
// linear sharpness the CPU computed from the 0..1 strength
// (`render::postfx::effects::taa::sharpen::rcas_sharpness`); a higher value sharpens
// more. The no-op case (strength 0) never records this pass, so the frame stays
// byte-identical.

#import fullscreen
#import numeric

struct RcasParams {
    // x: the RCAS sharpness scalar (FSR's `con.x`); y, z, w: unused.
    knobs: vec4<f32>,
};

@group(0) @binding(0) var<uniform> rcas: RcasParams;
@group(0) @binding(1) var t_color: texture_2d<f32>;

// FSR's peak-range limit (`FSR_RCAS_LIMIT`, 0.25 - 1/16).
const RCAS_LIMIT: f32 = 0.1875;

@vertex
fn vs_fullscreen(@builtin(vertex_index) vid: u32) -> @builtin(position) vec4<f32> {
    return fullscreen::triangle(vid);
}

// The display colour at `p`, clamped into the image and to finite `[0, 1]`, so one
// bad pixel never poisons the sharpen.
fn load(p: vec2<i32>) -> vec4<f32> {
    let last = vec2<i32>(textureDimensions(t_color)) - 1;
    let c = textureLoad(t_color, clamp(p, vec2<i32>(0), last), 0);
    let rgb = select(vec3<f32>(0.0), clamp(c.rgb, vec3<f32>(0.0), vec3<f32>(1.0)), numeric::finite3(c.rgb));
    return vec4<f32>(rgb, c.a);
}

@fragment
fn fs_rcas(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let p = vec2<i32>(pos.xy);
    //   b
    // d e f
    //   h
    let b = load(p + vec2<i32>(0, -1)).rgb;
    let d = load(p + vec2<i32>(-1, 0)).rgb;
    let centre = load(p);
    let e = centre.rgb;
    let f = load(p + vec2<i32>(1, 0)).rgb;
    let h = load(p + vec2<i32>(0, 1)).rgb;

    // Per-channel min/max of the ring {b, d, f, h}.
    let mn4 = min(min(b, d), min(f, h));
    let mx4 = max(max(b, d), max(f, h));

    // The sharpening lobe each channel's local range allows (FSR's RcasF); the
    // tightest channel wins, so no channel over- or under-shoots.
    let hit_min = min(mn4, e) / max(4.0 * mx4, vec3<f32>(1e-4));
    let hit_max = (vec3<f32>(1.0) - max(mx4, e)) / min(4.0 * mn4 - 4.0, vec3<f32>(-1e-4));
    let lobe_rgb = max(-hit_min, hit_max);
    let lobe = max(-RCAS_LIMIT, min(max(max(lobe_rgb.r, lobe_rgb.g), lobe_rgb.b), 0.0)) * rcas.knobs.x;

    // Resolve the cross blended in by the lobe.
    let out = (lobe * (b + d + h + f) + e) / (4.0 * lobe + 1.0);
    let safe = select(e, clamp(out, vec3<f32>(0.0), vec3<f32>(1.0)), numeric::finite3(out));
    return vec4<f32>(safe, centre.a);
}
