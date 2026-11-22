#define_import_path postfx::taa::io

#import numeric

// `taa::params::TaaParams::new` fills this, field for field.
struct TaaParams {
    // xy: the history's size in pixels (the output's when upscaling); zw: its reciprocal
    size: vec4<f32>,
    // x: frame influence; y: variance clamp scale; z: 1 when the history holds this view;
    // w: still below this many history pixels a frame
    knobs: vec4<f32>,
    // xy: the current frame's size in pixels; zw: its reciprocal
    scene: vec4<f32>,
    // xy: the jitter in scene pixels, NDC's y up; z: 1 when a translucent wrote the
    // reactive mask this frame
    jitter: vec4<f32>,
    // URP's `DoTemporalAA` arguments: x clamp (0 cross, 2 variance, 3 clipping), y motion
    // (0 own, 1 cross, 2 3×3), z history (2 bicubic, else bilinear), w central filter (1 on)
    levels: vec4<f32>,
    // x: a moving pixel's frame influence, at y history pixels a frame; z, w: the relative
    // distance gaps where the history starts to fade and is gone
    rejection: vec4<f32>,
    // x: a flicker's share kept per frame; y: the times its recurring flicker the
    // history's luma may sit outside the box
    flicker: vec4<f32>,
    // Per camera, by velocity `.w` (clamped to the last): xy, the view offset per
    // NDC unit at view depth 1 (perspective) or any (orthographic); z, 1 for perspective.
    lenses: array<vec4<f32>, MAX_CAMERAS>,
};

const MAX_CAMERAS: u32 = 8u;

@group(0) @binding(0) var<uniform> taa: TaaParams;
@group(0) @binding(1) var t_current: texture_2d<f32>;
@group(0) @binding(2) var t_history: texture_2d<f32>;
@group(0) @binding(3) var t_velocity: texture_2d<f32>;
@group(0) @binding(4) var s_linear: sampler;
@group(0) @binding(5) var t_surface_history: texture_2d<f32>;
// The topmost camera that drew a translucent, plus the share of the pixel it laid down
// (`reactive.wgsl`); 0 where none drew.
@group(0) @binding(6) var t_reactive: texture_2d<f32>;
@group(0) @binding(7) var t_flicker_history: texture_2d<f32>;

const MAX_HDR: f32 = 65504.0;

fn current(p: vec2<i32>) -> vec4<f32> {
    let last = vec2<i32>(textureDimensions(t_current)) - 1;
    let c = textureLoad(t_current, clamp(p, vec2<i32>(0), last), 0);
    let rgb = select(vec3<f32>(0.0), clamp(c.rgb, vec3<f32>(0.0), vec3<f32>(MAX_HDR)), numeric::finite3(c.rgb));
    return vec4<f32>(rgb, c.a);
}
