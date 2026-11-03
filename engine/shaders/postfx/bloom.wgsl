// engine/shaders/postfx/bloom.wgsl — bloom's mip chain, before the composite
// (`render::postfx::effects::bloom`), after Jimenez's *Next-Generation Post Processing
// in Call of Duty: Advanced Warfare* (SIGGRAPH 2014) with Unity URP's knobs:
// - `fs_prefilter`: the scene down to the chain's first mip with the 13-tap filter, a
//   Karis average (against fireflies), then URP's soft-knee threshold;
// - `fs_downsample`: each mip into the next, half its size, with the 13-tap filter;
// - `fs_upsample`: back up, a 3×3 tent of the mip below blended over this one's by
//   the scatter, as URP's `lerp(highMip, lowMip, Scatter)`.
// The composite adds the top of the chain (`postfx/effects/bloom.wgsl`).

#import fullscreen
#import postfx::grade::luminance

// Mirrors the Rust `BloomParams`.
struct BloomParams {
    // x: the threshold, y: its knee (half the threshold, as URP fixes it), z: the
    // upsample's blend, URP's `lerp(0.05, 0.95, scatter)`, w: the clamp on the input
    knobs: vec4<f32>,
    // xy: one texel of the chain's first mip (the prefilter's step), zw: unused
    texel: vec4<f32>,
};

@group(0) @binding(0) var<uniform> params: BloomParams;
// The pass's input: the scene (prefilter), the mip above (downsample), or the mip
// below (upsample).
@group(0) @binding(1) var t_color: texture_2d<f32>;
@group(0) @binding(2) var s_color: sampler;
// The upsample's own mip, which the mip below blends over; unused elsewhere.
@group(0) @binding(3) var t_high: texture_2d<f32>;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_fullscreen(@builtin(vertex_index) vid: u32) -> VsOut {
    var out: VsOut;
    out.uv = fullscreen::uv(vid);
    out.pos = fullscreen::triangle_down(vid);
    return out;
}

// Every sample is `textureSampleLevel` at LOD 0: the targets have no mips, and an
// implicit-LOD sample is a gradient instruction, which D3D12 refuses in a loop and
// derivatives forbid off uniform control flow.
fn tap(uv: vec2<f32>) -> vec3<f32> {
    return textureSampleLevel(t_color, s_color, uv, 0.0).rgb;
}

// A NaN channel, by its bits: `x != x` may be folded away by a compiler.
fn is_nan(x: f32) -> bool {
    return (bitcast<u32>(x) & 0x7fffffffu) > 0x7f800000u;
}

// One scene sample made safe for the chain: NaN is black, inf and anything above the
// clamp is the clamp (URP's `ClampMax`), negatives are zero. Only the prefilter reads
// the scene, so nothing it lets through can spread down the chain.
fn safe_tap(uv: vec2<f32>) -> vec3<f32> {
    let c = tap(uv);
    let nan = is_nan(c.r) || is_nan(c.g) || is_nan(c.b);
    let clamped = clamp(c, vec3<f32>(0.0), vec3<f32>(params.knobs.w));
    return select(clamped, vec3<f32>(0.0), nan);
}

// Jimenez's 13 taps around `uv`, one destination texel `step` apart (a 2× reduction
// reads the source at ±1 and ±2 of its texels), as five boxes of four: the inner one
// (weight 0.5) and the four corner ones (0.125 each).
struct Boxes {
    inner: vec3<f32>,
    a: vec3<f32>,
    b: vec3<f32>,
    c: vec3<f32>,
    d: vec3<f32>,
};

fn boxes(uv: vec2<f32>, step: vec2<f32>, safe: bool) -> Boxes {
    var s: array<vec3<f32>, 13>;
    var offsets = array<vec2<f32>, 13>(
        vec2<f32>(-1.0, -1.0), vec2<f32>(0.0, -1.0), vec2<f32>(1.0, -1.0),
        vec2<f32>(-0.5, -0.5), vec2<f32>(0.5, -0.5),
        vec2<f32>(-1.0, 0.0), vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0),
        vec2<f32>(-0.5, 0.5), vec2<f32>(0.5, 0.5),
        vec2<f32>(-1.0, 1.0), vec2<f32>(0.0, 1.0), vec2<f32>(1.0, 1.0),
    );
    for (var i = 0; i < 13; i = i + 1) {
        let at = uv + offsets[i] * step;
        if (safe) {
            s[i] = safe_tap(at);
        } else {
            s[i] = tap(at);
        }
    }
    var out: Boxes;
    out.inner = (s[3] + s[4] + s[8] + s[9]) * 0.25;
    out.a = (s[0] + s[1] + s[5] + s[6]) * 0.25;
    out.b = (s[1] + s[2] + s[6] + s[7]) * 0.25;
    out.c = (s[5] + s[6] + s[10] + s[11]) * 0.25;
    out.d = (s[6] + s[7] + s[11] + s[12]) * 0.25;
    return out;
}

// A box's weight in the Karis average: its share, over one plus its luminance, so a
// lone bright texel counts as one bright box, not as all of its energy.
fn karis(share: f32, c: vec3<f32>) -> f32 {
    return share / (1.0 + luminance(c));
}

// URP's threshold: the brightest channel against the threshold, with a quadratic
// soft knee of half its width either side.
fn threshold(c: vec3<f32>) -> vec3<f32> {
    let t = params.knobs.x;
    let knee = params.knobs.y;
    let brightness = max(c.r, max(c.g, c.b));
    var soft = clamp(brightness - t + knee, 0.0, 2.0 * knee);
    soft = soft * soft / (4.0 * knee + 1e-4);
    let multiplier = max(brightness - t, soft) / max(brightness, 1e-4);
    return max(c * multiplier, vec3<f32>(0.0));
}

@fragment
fn fs_prefilter(in: VsOut) -> @location(0) vec4<f32> {
    let b = boxes(in.uv, params.texel.xy, true);
    let w0 = karis(0.5, b.inner);
    let wa = karis(0.125, b.a);
    let wb = karis(0.125, b.b);
    let wc = karis(0.125, b.c);
    let wd = karis(0.125, b.d);
    let sum = b.inner * w0 + b.a * wa + b.b * wb + b.c * wc + b.d * wd;
    let c = sum / (w0 + wa + wb + wc + wd);
    return vec4<f32>(threshold(c), 1.0);
}

@fragment
fn fs_downsample(in: VsOut) -> @location(0) vec4<f32> {
    let step = 2.0 / vec2<f32>(textureDimensions(t_color));
    let b = boxes(in.uv, step, false);
    let c = b.inner * 0.5 + (b.a + b.b + b.c + b.d) * 0.125;
    return vec4<f32>(c, 1.0);
}

@fragment
fn fs_upsample(in: VsOut) -> @location(0) vec4<f32> {
    let t = 1.0 / vec2<f32>(textureDimensions(t_color));
    var low = tap(in.uv) * 4.0;
    low += (tap(in.uv + vec2<f32>(-t.x, 0.0)) + tap(in.uv + vec2<f32>(t.x, 0.0))) * 2.0;
    low += (tap(in.uv + vec2<f32>(0.0, -t.y)) + tap(in.uv + vec2<f32>(0.0, t.y))) * 2.0;
    low += tap(in.uv - t) + tap(in.uv + t);
    low += tap(in.uv + vec2<f32>(-t.x, t.y)) + tap(in.uv + vec2<f32>(t.x, -t.y));
    let high = textureSampleLevel(t_high, s_color, in.uv, 0.0).rgb;
    return vec4<f32>(mix(high, low / 16.0, params.knobs.z), 1.0);
}
