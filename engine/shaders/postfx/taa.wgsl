// engine/shaders/postfx/taa.wgsl — temporal anti-aliasing's resolve (#1702), URP's
// `TemporalAA.hlsl` at its default quality (*High*: `DoTemporalAA(2, 2, 2, 0)`):
//   - the neighbourhood: the current pixel's 3×3 in YCoCg, its min/max box tightened
//     to mean ± `clamp scale` × the standard deviation (variance clamping);
//   - the motion: the velocity of the 3×3's nearest surface (so an edge reprojects
//     with what is in front of it);
//   - the history: a 5-tap bicubic (Catmull-Rom) read at the reprojected uv, dropped
//     when it lands off the image, on a cut, or when it isn't finite;
//   - the blend: history clamped into the box, then lerped toward the current pixel
//     by the frame influence in URP's perceptual space (each colour over 1 + luma), so
//     a bright highlight cannot dominate the average.
// Its output is both what the rest of the chain reads and next frame's history. The
// current pixel is the jittered frame's own, not re-filtered (URP's `High`; its
// `VeryHigh` adds the filter), so a still frame converges to the jitter's average:
// a supersampled pixel.
//
// Units. The velocity target holds `ndc_now - ndc_prev` in `.xy`, both unjittered,
// and in `.z` the view depth (the sky: 65504), so the nearest surface is the
// smallest `.z`.

#import fullscreen

struct TaaParams {
    // xy: the view's size in pixels; zw: one over it
    size: vec4<f32>,
    // x: the frame influence (the current frame's weight), y: the variance clamp's
    // scale, z: 1 when the history holds a frame of this view, w: unused
    knobs: vec4<f32>,
};

@group(0) @binding(0) var<uniform> taa: TaaParams;
@group(0) @binding(1) var t_current: texture_2d<f32>;
@group(0) @binding(2) var t_history: texture_2d<f32>;
@group(0) @binding(3) var t_velocity: texture_2d<f32>;
@group(0) @binding(4) var s_linear: sampler;

// A half float's largest value: what the HDR target can hold.
const MAX_HDR: f32 = 65504.0;

@vertex
fn vs_fullscreen(@builtin(vertex_index) vid: u32) -> @builtin(position) vec4<f32> {
    return fullscreen::triangle(vid);
}

// Whether every component is finite: an infinity or NaN has every exponent bit set.
// Checked on the bits, since a shader compiler may fold away a NaN compare.
fn finite(v: vec3<f32>) -> bool {
    let exponent = bitcast<vec3<u32>>(v) & vec3<u32>(0x7f800000u);
    return all(exponent != vec3<u32>(0x7f800000u));
}

fn rgb_to_ycocg(c: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(
        dot(c, vec3<f32>(0.25, 0.5, 0.25)),
        dot(c, vec3<f32>(0.5, 0.0, -0.5)),
        dot(c, vec3<f32>(-0.25, 0.5, -0.25)),
    );
}

fn ycocg_to_rgb(c: vec3<f32>) -> vec3<f32> {
    let t = c.x - c.z;
    return vec3<f32>(t + c.y, c.x + c.z, t - c.y);
}

// The current frame at pixel `p` (clamped to the image): a non-finite pixel reads
// black, so one bad pixel never enters the box or the history.
fn current(p: vec2<i32>) -> vec4<f32> {
    let last = vec2<i32>(textureDimensions(t_current)) - 1;
    let c = textureLoad(t_current, clamp(p, vec2<i32>(0), last), 0);
    let rgb = select(vec3<f32>(0.0), clamp(c.rgb, vec3<f32>(0.0), vec3<f32>(MAX_HDR)), finite(c.rgb));
    return vec4<f32>(rgb, c.a);
}

// The NDC motion of the nearest surface in `p`'s 3×3 (URP's motion quality 2); none
// when it isn't finite.
fn nearest_motion(p: vec2<i32>) -> vec2<f32> {
    let last = vec2<i32>(textureDimensions(t_velocity)) - 1;
    var depth = 3.0e38;
    var motion = vec2<f32>(0.0);
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            let t = textureLoad(t_velocity, clamp(p + vec2<i32>(x, y), vec2<i32>(0), last), 0);
            if (t.z < depth) {
                depth = t.z;
                motion = t.xy;
            }
        }
    }
    return select(vec2<f32>(0.0), motion, finite(vec3<f32>(motion, 0.0)));
}

// The history at `uv`, Catmull-Rom from 5 bilinear taps (Jimenez's *Filmic SMAA*,
// as URP's `SampleBicubic5TapHalf`): sharper than one bilinear read, so a moving
// image does not soften frame after frame.
fn history_bicubic(uv: vec2<f32>) -> vec3<f32> {
    let size = taa.size.xy;
    let pos = uv * size;
    let center = floor(pos - 0.5) + 0.5;
    let f = pos - center;
    let w0 = f * (-0.5 + f * (1.0 - 0.5 * f));
    let w1 = 1.0 + f * f * (-2.5 + 1.5 * f);
    let w2 = f * (0.5 + f * (2.0 - 1.5 * f));
    let w3 = f * f * (-0.5 + 0.5 * f);
    let w12 = w1 + w2;
    let at0 = (center - 1.0) * taa.size.zw;
    let at3 = (center + 2.0) * taa.size.zw;
    let at12 = (center + w2 / w12) * taa.size.zw;
    let a = textureSampleLevel(t_history, s_linear, vec2<f32>(at12.x, at0.y), 0.0).rgb;
    let b = textureSampleLevel(t_history, s_linear, vec2<f32>(at0.x, at12.y), 0.0).rgb;
    let c = textureSampleLevel(t_history, s_linear, at12, 0.0).rgb;
    let d = textureSampleLevel(t_history, s_linear, vec2<f32>(at3.x, at12.y), 0.0).rgb;
    let e = textureSampleLevel(t_history, s_linear, vec2<f32>(at12.x, at3.y), 0.0).rgb;
    let wa = w12.x * w0.y;
    let wb = w0.x * w12.y;
    let wc = w12.x * w12.y;
    let wd = w3.x * w12.y;
    let we = w12.x * w3.y;
    return (a * wa + b * wb + c * wc + d * wd + e * we) / (wa + wb + wc + wd + we);
}

@fragment
fn fs_resolve(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let p = vec2<i32>(pos.xy);
    let here = current(p);
    let center = rgb_to_ycocg(here.rgb);

    // The neighbourhood box and its moments.
    var lo = center;
    var hi = center;
    var m1 = center;
    var m2 = center * center;
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            if (x != 0 || y != 0) {
                let c = rgb_to_ycocg(current(p + vec2<i32>(x, y)).rgb);
                lo = min(lo, c);
                hi = max(hi, c);
                m1 += c;
                m2 += c * c;
            }
        }
    }
    let mean = m1 / 9.0;
    let deviation = sqrt(abs(m2 / 9.0 - mean * mean));
    let spread = taa.knobs.y * deviation;
    lo = max(lo, mean - spread);
    hi = min(hi, mean + spread);

    // Last frame's resolve where this pixel's surface was. NDC y points up, uv down.
    let uv = pos.xy * taa.size.zw;
    let prev_uv = uv - nearest_motion(p) * vec2<f32>(0.5, -0.5);
    let sampled = history_bicubic(prev_uv);
    let inside = all(abs(prev_uv - 0.5) <= vec2<f32>(0.5));
    if (taa.knobs.z < 0.5 || !inside || !finite(sampled)) {
        // No history to blend: the current frame alone, exactly.
        return here;
    }
    let history = clamp(rgb_to_ycocg(max(sampled, vec3<f32>(0.0))), lo, hi);

    // Lerp in perceptual space: each colour over 1 + its luma, and back.
    let a = history / (1.0 + history.x);
    let b = center / (1.0 + center.x);
    let mixed = mix(a, b, taa.knobs.x);
    let rgb = ycocg_to_rgb(mixed / max(1.0 - mixed.x, 1.0 / MAX_HDR));
    let out = clamp(rgb, vec3<f32>(0.0), vec3<f32>(MAX_HDR));
    return vec4<f32>(select(here.rgb, out, finite(rgb)), here.a);
}
