// engine/shaders/postfx/taa/io.wgsl — what TAA's resolve (`postfx/taa.wgsl`)
// reads: its uniform (`render::postfx::effects::taa::params::TaaParams`), its
// bindings (`taa::pass`'s layouts: 0 to 6 for every resolve, 7 for the `_held`
// ones only) and the current frame, read safely.

#define_import_path postfx::taa::io

#import numeric

struct TaaParams {
    // xy: the history's size in pixels (the scene's, or the output's when
    // upscaling); zw: one over it
    size: vec4<f32>,
    // x: the frame influence (the current frame's weight), y: the variance clamp's
    // scale, z: 1 when the history holds a frame of this view, w: the history pixels
    // a frame of motion below which a pixel is still
    knobs: vec4<f32>,
    // xy: the scene's size in pixels (the current frame's); zw: one over it
    scene: vec4<f32>,
    // xy: this frame's jitter in scene pixels (`jitter::pixels`, NDC's y up); z: 1
    // when a translucent wrote the reactive mask this frame (#1845); w: unused
    jitter: vec4<f32>,
    // The quality level's steps (#1847, `params::levels`), URP's `DoTemporalAA`
    // arguments: x clamp (0 cross, 2 variance, 3 clipping), y motion (0 own, 1 cross,
    // 2 3×3), z history (below 2 bilinear, 2 bicubic), w central filtering (1 on)
    levels: vec4<f32>,
    // x: a moving pixel's frame influence, y: the history pixels a frame it applies
    // at; z, w: the relative depth gap the history starts to fade at, and is gone
    rejection: vec4<f32>,
    // The anti-flicker's hold (#2228, `params::FLICKER_DECAY`), read by the `_held`
    // entry points only: x, what a pixel's flicker keeps of itself a frame; y, how
    // many times its recurring flicker the history's luma may sit outside the box
    // (`FLICKER_HOLD` × the camera's anti-flicker); zw: unused
    flicker: vec4<f32>,
    // Each camera of the stack by its index (the velocity's `.w`; a camera past the
    // last slot reads the last): xy, the view-space offset per NDC unit at one unit
    // of view depth (perspective) or at any (orthographic); z, 1 for perspective.
    lenses: array<vec4<f32>, MAX_CAMERAS>,
};

// How many cameras' lenses the resolve holds: `params::MAX_CAMERAS`.
const MAX_CAMERAS: u32 = 8u;

@group(0) @binding(0) var<uniform> taa: TaaParams;
@group(0) @binding(1) var t_current: texture_2d<f32>;
@group(0) @binding(2) var t_history: texture_2d<f32>;
@group(0) @binding(3) var t_velocity: texture_2d<f32>;
@group(0) @binding(4) var s_linear: sampler;
// Last resolve's surface per history pixel, each pixel's own (`ResolveOut::surface`):
// its distance from its camera in `.x`, that camera's index in `.y`.
@group(0) @binding(5) var t_surface_history: texture_2d<f32>;
// This frame's reactive mask at the scene's size (#1845, `reactive.wgsl`): the
// topmost camera that drew a translucent there, plus how much of the pixel that
// camera's translucents laid down (below 1); 0 where none drew.
@group(0) @binding(6) var t_reactive: texture_2d<f32>;
// Last resolve's flicker per history pixel (`HeldOut::flicker`, #2228,
// `taa_flicker.wgsl`), bound for the `_held` entry points only: its frame's
// perceptual luma, its last step (signed), its flicker and how much of it recurs.
@group(0) @binding(7) var t_flicker_history: texture_2d<f32>;

// A half float's largest value: what the HDR target can hold.
const MAX_HDR: f32 = 65504.0;

// The current frame at pixel `p` (clamped to the image): a non-finite pixel reads
// black, so one bad pixel never enters the box or the history.
fn current(p: vec2<i32>) -> vec4<f32> {
    let last = vec2<i32>(textureDimensions(t_current)) - 1;
    let c = textureLoad(t_current, clamp(p, vec2<i32>(0), last), 0);
    let rgb = select(vec3<f32>(0.0), clamp(c.rgb, vec3<f32>(0.0), vec3<f32>(MAX_HDR)), numeric::finite3(c.rgb));
    return vec4<f32>(rgb, c.a);
}
