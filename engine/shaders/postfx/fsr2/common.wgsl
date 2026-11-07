// engine/shaders/postfx/fsr2/common.wgsl — FSR2's shared code (#1848): AMD FidelityFX
// Super Resolution 2.2.1's `ffx_fsr2_common.h` and the parts of `ffx_core` and its
// callbacks every pass reads, ported by hand to WGSL. Copyright (c) 2022-2023
// Advanced Micro Devices, Inc.; MIT, the notice in `LICENSE.txt` beside this file.
//
// The port runs FSR2's fp32, non-wave permutation with the options rusty's inputs
// call for: HDR colour input, render-size (low resolution) motion vectors that are
// not jittered, no sharpening (#1847's RCAS runs after the composite instead), no
// transparency-and-composition mask, and the engine's exposure (FSR2's auto
// exposure off). One more departure, here once: FSR2's *device depth* is the view
// depth itself, the velocity target's `.z` (the stack's own cameras', the sky
// 65504), so `GetViewSpaceDepth` is the identity and the depth is never inverted.
// A camera stack's cameras each wrote their own projection's device depth into one
// buffer, which no single transform undoes; their view depths are comparable.

#define_import_path postfx::fsr2::common

#import numeric

struct Fsr2Params {
    // xy: the render (scene) size in pixels; zw: the display (output) size
    sizes: vec4<f32>,
    // xy: the jitter in render pixels with FSR2's sign (y down: a pixel's sample
    // lies at its centre minus it); zw: the motion-vector scale, the velocity
    // target's NDC motion to FSR2's uv motion (`MotionVectorScale`)
    jitter: vec4<f32>,
    // x: the frame index (0 on a reset), y: the jitter phase count
    // (`JitterSequenceLength`), z: 1 when a translucent wrote the reactive mask,
    // w: unused
    frame: vec4<f32>,
    // x: the fixed EV, y: 1 when the exposure is the meter's adapted EV; zw: the
    // base camera's view-space offset per NDC unit at depth 1 (FSR2's
    // `deviceToViewDepth[2..3]`)
    exposure: vec4<f32>,
};

@group(0) @binding(0) var<uniform> fsr: Fsr2Params;
// The exposure meter's adapted state (`ExposureMeter::state`): `.x` the EV, a frame
// late, as everything that reads it.
@group(0) @binding(1) var<uniform> exposure_state: vec4<f32>;

const FP16_MAX: f32 = 65504.0;
const EPSILON: f32 = 1e-3;
const TONEMAP_EPSILON: f32 = 1.0 / 65504.0;
const FLT_MAX: f32 = 3.402823466e+38;
const RECONSTRUCTED_DEPTH_BILINEAR_WEIGHT_THRESHOLD: f32 = 0.01;
const UPSAMPLE_LANCZOS_WEIGHT_SCALE: f32 = 1.0 / 12.0;
const MAX_ACCUMULATION_LANCZOS_WEIGHT: f32 = 1.0;
const AVERAGE_LANCZOS_WEIGHT_PER_FRAME: f32 = 0.74 / 12.0;
// The farthest view depth: the velocity target's sky, FSR2's far plane.
const FAR_DEPTH: f32 = 65504.0;
// The luminance pyramid's level the locks read (`FFX_FSR2_SHADING_CHANGE_MIP_LEVEL`):
// a texel per 32×32 render pixels (`2 << 4`).
const LUMA_MIP_DIV: f32 = 32.0;
const PI: f32 = 3.141592653589793;

fn render_size() -> vec2<f32> {
    return fsr.sizes.xy;
}

fn render_size_i() -> vec2<i32> {
    return vec2<i32>(fsr.sizes.xy);
}

fn display_size() -> vec2<f32> {
    return fsr.sizes.zw;
}

fn display_size_i() -> vec2<i32> {
    return vec2<i32>(fsr.sizes.zw);
}

fn jitter() -> vec2<f32> {
    return fsr.jitter.xy;
}

fn downscale_factor() -> vec2<f32> {
    return fsr.sizes.xy / fsr.sizes.zw;
}

fn is_reset_frame() -> bool {
    return fsr.frame.x == 0.0;
}

// FSR2's `Exposure()`: the engine's exposure as a multiplier; a value that is not a
// positive number reads 1, as FSR2 reads a zero.
fn exposure() -> f32 {
    let ev = select(fsr.exposure.x, exposure_state.x, fsr.exposure.y > 0.5);
    let e = exp2(ev);
    return select(1.0, e, e > 0.0 && e <= FLT_MAX);
}

// A colour read from the scene: non-finite reads black, so one bad pixel never
// enters the history (TAA's rule, `current` in `postfx/taa.wgsl`).
fn finite_rgb(c: vec3<f32>) -> vec3<f32> {
    return select(vec3<f32>(0.0), c, numeric::finite3(c));
}

fn saturate(x: f32) -> f32 {
    return clamp(x, 0.0, 1.0);
}

fn rgb_to_ycocg(c: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(
        0.25 * c.r + 0.5 * c.g + 0.25 * c.b,
        0.5 * c.r - 0.5 * c.b,
        -0.25 * c.r + 0.5 * c.g - 0.25 * c.b,
    );
}

fn ycocg_to_rgb(c: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(c.x + c.y - c.z, c.x + c.z, c.x - c.y - c.z);
}

fn rgb_to_luma(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
}

fn rgb_to_perceived_luma(c: vec3<f32>) -> f32 {
    let l = rgb_to_luma(c);
    var perceived: f32;
    if (l <= 216.0 / 24389.0) {
        perceived = l * (24389.0 / 27.0);
    } else {
        perceived = pow(l, 1.0 / 3.0) * 116.0 - 16.0;
    }
    return perceived * 0.01;
}

fn tonemap(c: vec3<f32>) -> vec3<f32> {
    return c / (max(max(0.0, c.r), max(c.g, c.b)) + 1.0);
}

fn inverse_tonemap(c: vec3<f32>) -> vec3<f32> {
    return c / max(TONEMAP_EPSILON, 1.0 - max(c.r, max(c.g, c.b)));
}

fn min_divided_by_max(a: f32, b: f32) -> f32 {
    let m = max(a, b);
    return select(0.0, min(a, b) / m, m != 0.0);
}

// FSR2's `PrepareRgb` with a pre-exposure of 1.
fn prepare_rgb(c: vec3<f32>, e: f32) -> vec3<f32> {
    return clamp(c * e, vec3<f32>(0.0), vec3<f32>(FP16_MAX));
}

fn unprepare_rgb(c: vec3<f32>, e: f32) -> vec3<f32> {
    return c / e;
}

fn is_on_screen(p: vec2<i32>, size: vec2<i32>) -> bool {
    return all(p >= vec2<i32>(0)) && all(p < size);
}

// `ClampLoad`: `p + offset`, clamped on the side the offset points to.
fn clamp_load(p: vec2<i32>, offset: vec2<i32>, size: vec2<i32>) -> vec2<i32> {
    var r = p + offset;
    r.x = select(r.x, max(r.x, 0), offset.x < 0);
    r.x = select(r.x, min(r.x, size.x - 1), offset.x > 0);
    r.y = select(r.y, max(r.y, 0), offset.y < 0);
    r.y = select(r.y, min(r.y, size.y - 1), offset.y > 0);
    return r;
}

fn clamp_uv(uv: vec2<f32>, texture_size: vec2<i32>, resource_size: vec2<i32>) -> vec2<f32> {
    let at = uv * vec2<f32>(texture_size);
    let clamped = max(vec2<f32>(0.5), min(at, vec2<f32>(texture_size) - vec2<f32>(0.5)));
    return clamped / vec2<f32>(resource_size);
}

fn compute_hr_pos_from_lr_pos(lr: vec2<i32>) -> vec2<i32> {
    let jittered = vec2<f32>(lr) + 0.5 - jitter();
    let in_hr = (jittered / render_size()) * display_size();
    return vec2<i32>(floor(in_hr));
}

fn compute_ndc(px: vec2<f32>, size: vec2<i32>) -> vec2<f32> {
    return px / vec2<f32>(size) * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0);
}

// `GetViewSpaceDepth`: the identity, since FSR2's device depth is the view depth
// here (the header).
fn view_space_depth(d: f32) -> f32 {
    return d;
}

fn view_space_position(px: vec2<i32>, size: vec2<i32>, d: f32) -> vec3<f32> {
    let z = view_space_depth(d);
    let ndc = compute_ndc(vec2<f32>(px), size);
    return vec3<f32>(fsr.exposure.z * ndc.x * z, fsr.exposure.w * ndc.y * z, z);
}

// `BilinearSamplingData`: the base texel and the four taps' weights.
struct Bilinear {
    base: vec2<i32>,
    weights: vec4<f32>,
};

const BILINEAR_OFFSETS: array<vec2<i32>, 4> = array<vec2<i32>, 4>(
    vec2<i32>(0, 0),
    vec2<i32>(1, 0),
    vec2<i32>(0, 1),
    vec2<i32>(1, 1),
);

fn bilinear_sampling(uv: vec2<f32>, size: vec2<i32>) -> Bilinear {
    let px = uv * vec2<f32>(size) - vec2<f32>(0.5);
    let f = fract(px);
    let w = vec4<f32>(
        (1.0 - f.x) * (1.0 - f.y),
        f.x * (1.0 - f.y),
        (1.0 - f.x) * f.y,
        f.x * f.y,
    );
    return Bilinear(vec2<i32>(floor(px)), w);
}

// `Lanczos2` (reference), its window clamped to 2.
fn lanczos2(x_in: f32) -> f32 {
    let x = min(abs(x_in), 2.0);
    if (abs(x) < EPSILON) {
        return 1.0;
    }
    return (sin(PI * x) / (PI * x)) * (sin(0.5 * PI * x) / (0.5 * PI * x));
}

// `Lanczos2ApproxSq`: the approximate window on the squared distance.
fn lanczos2_approx_sq(x2_in: f32) -> f32 {
    let x2 = min(x2_in, 4.0);
    let a = (2.0 / 5.0) * x2 - 1.0;
    let b = (1.0 / 4.0) * x2 - 1.0;
    return ((25.0 / 16.0) * a * a - (25.0 / 16.0 - 1.0)) * (b * b);
}
