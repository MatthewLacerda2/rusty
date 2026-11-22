// what TAA's resolve (`postfx/taa.wgsl`) reads: its uniform
// (`render::postfx::effects::taa::params::TaaParams`), its bindings (`taa::pass`'s layouts: 0
// to 6 for every resolve, 7 for the `_held` ones only) and the current frame, read safely.

#define_import_path postfx::taa::io

#import numeric

struct TaaParams {
    size: vec4<f32>,
    knobs: vec4<f32>,
    scene: vec4<f32>,
    // xy: this frame's jitter in scene pixels (`jitter::pixels`, NDC's y up); z: 1 when a
    // translucent wrote the reactive mask this frame; w: unused
    jitter: vec4<f32>,
    // The quality level's steps (`params::levels`), URP's `DoTemporalAA` arguments: x clamp (0
    // cross, 2 variance, 3 clipping), y motion (0 own, 1 cross, 2 3×3), z history (below 2
    // bilinear, 2 bicubic), w central filtering (1 on)
    levels: vec4<f32>,
    rejection: vec4<f32>,
    flicker: vec4<f32>,
    // Each camera of the stack by its index (the velocity's `.w`; a camera past the last slot
    // reads the last): xy, the view-space offset per NDC unit at one unit of view depth
    // (perspective) or at any (orthographic); z, 1 for perspective.
    lenses: array<vec4<f32>, MAX_CAMERAS>,
};

const MAX_CAMERAS: u32 = 8u;

@group(0) @binding(0) var<uniform> taa: TaaParams;
@group(0) @binding(1) var t_current: texture_2d<f32>;
@group(0) @binding(2) var t_history: texture_2d<f32>;
@group(0) @binding(3) var t_velocity: texture_2d<f32>;
@group(0) @binding(4) var s_linear: sampler;
@group(0) @binding(5) var t_surface_history: texture_2d<f32>;
// This frame's reactive mask at the scene's size (`reactive.wgsl`): the topmost camera that
// drew a translucent there, plus how much of the pixel that camera's translucents laid down
// (below 1); 0 where none drew.
@group(0) @binding(6) var t_reactive: texture_2d<f32>;
@group(0) @binding(7) var t_flicker_history: texture_2d<f32>;

const MAX_HDR: f32 = 65504.0;

fn current(p: vec2<i32>) -> vec4<f32> {
    let last = vec2<i32>(textureDimensions(t_current)) - 1;
    let c = textureLoad(t_current, clamp(p, vec2<i32>(0), last), 0);
    let rgb = select(vec3<f32>(0.0), clamp(c.rgb, vec3<f32>(0.0), vec3<f32>(MAX_HDR)), numeric::finite3(c.rgb));
    return vec4<f32>(rgb, c.a);
}
