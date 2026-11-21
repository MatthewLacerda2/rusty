// engine/shaders/postfx/taa.wgsl — temporal anti-aliasing's resolve, URP's
// `TemporalAA.hlsl`; at its default quality (*High*: `DoTemporalAA(2, 2, 2, 0)`):
//   - the neighbourhood: the current pixel's 3×3 in YCoCg, its min/max box tightened
//     to mean ± `clamp scale` × the standard deviation (variance clamping);
//   - the motion: the velocity of the 3×3's frontmost surface (so an edge
//     reprojects with what is in front of it: a viewmodel's over the world's);
//   - the history: a 5-tap bicubic (Catmull-Rom) read at the reprojected uv, dropped
//     when it lands off the image, on a cut, or when it isn't finite;
//   - the blend: history clamped into the box, then lerped toward the current pixel
//     by the frame influence in URP's perceptual space (each colour over 1 + luma), so
//     a bright highlight cannot dominate the average.
// The other levels run URP's other `DoTemporalAA` arguments, `taa.levels`
// (`params::levels`): a cross box, less or no motion dilation, a bilinear history,
// clipping toward the box's centre and a re-filtered current pixel.
// Beyond URP (`taa/params.rs`): the frame weighs more as the pixel moves, and
// the history fades where this pixel's surface lies farther from its camera than
// last frame's there, or a camera higher in the stack drew there and no longer does
// (a reveal). Where a translucent drew this frame (its reactive mask, FSR2's)
// the frame weighs more again, so a moving translucent, which writes no motion and
// is reprojected with what is behind it, keeps its detail.
// Its output is what the rest of the chain reads and next frame's history
// (colour, and its own surface: distance and camera, `ResolveOut`).
// The `_held` entry points are the same resolves with the camera's anti-flicker on
// (HDRP's, `taa_flicker.wgsl`): a still pixel whose frame keeps blinking
// under the jitter holds its history's luma against the next blink, which a clamp
// to this frame's own box would hand through, and each pixel's flicker is a third
// output (`HeldOut`). The plain ones are URP's resolve, reading and writing none of it.
// `fs_upscale` is the same resolve upscaling (TAAU,
// `render/postfx/effects/taa/upscale/`): a scaled view's scene-size frame into an
// output-size history, at the same place in the chain, so what follows it runs at
// the output's size. The current pixel is the jittered frame's own, not re-filtered
// (URP's `High`; its `VeryHigh` adds the filter), so a still frame converges to the
// jitter's average:
// a supersampled pixel.
//
// What the resolve reads (its uniform and bindings) is `taa/io.wgsl`; the surfaces it
// reprojects and tells a reveal by, `taa/surface.wgsl`.
//
// Units. The velocity target holds `ndc_now - ndc_prev` in `.xy`, both unjittered,
// in `.z` the view depth (the sky: 65504) and in `.w` the drawing camera's index in
// its stack, so the frontmost surface is the highest `.w`, then the smallest `.z`.

#import fullscreen
#import numeric
#import postfx::taa::io::{taa, t_current, t_history, s_linear, t_reactive, t_flicker_history, current, MAX_HDR}
#import postfx::taa::surface::{Front, frontmost, depth_mismatch}
#import postfx::taa_flicker
#import postfx::ycocg

struct ResolveOut { // what the resolve writes: next frame's history, colour and surface
    @location(0) color: vec4<f32>,
    @location(1) surface: vec2<f32>,
};

// What the resolve writes with the camera's anti-flicker on: `ResolveOut`'s,
// and the pixel's flicker, its frame's luma, then its step, flicker and recurring
// flicker (`taa_flicker`).
struct HeldOut {
    @location(0) color: vec4<f32>,
    @location(1) surface: vec2<f32>,
    @location(2) flicker: vec4<f32>,
};

@vertex
fn vs_fullscreen(@builtin(vertex_index) vid: u32) -> @builtin(position) vec4<f32> {
    return fullscreen::triangle(vid);
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

// A re-filter of the current pixel (URP's central filtering, `VeryHigh`): the
// 3×3's samples weighted by where each landed under the jitter ([`sample_weight`]),
// so the output pixel converges on the jittered average sooner. Lower levels take
// the point sample.
fn filtered_center(p: vec2<i32>) -> vec4<f32> {
    let shift = vec2<f32>(taa.jitter.x, -taa.jitter.y);
    var sum = vec3<f32>(0.0);
    var weights = 0.0;
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            let w = sample_weight(vec2<f32>(f32(x), f32(y)) - shift);
            sum += current(p + vec2<i32>(x, y)).rgb * w;
            weights += w;
        }
    }
    return vec4<f32>(sum / max(weights, 1e-6), current(p).a);
}

// One output pixel's frame, where it reads its history and its frontmost surface:
// what [`blend`] and [`blend_held`] take.
struct Pixel {
    frame: Frame,
    uv: vec2<f32>,
    front: Front,
};

@fragment
fn fs_resolve(@builtin(position) pos: vec4<f32>) -> ResolveOut {
    return blend(resolved(pos));
}

// `fs_resolve` with the camera's anti-flicker on.
@fragment
fn fs_resolve_held(@builtin(position) pos: vec4<f32>) -> HeldOut {
    return blend_held(resolved(pos));
}

// The scene-size resolve's [`Pixel`] at `pos`: the current pixel and its box.
fn resolved(pos: vec4<f32>) -> Pixel {
    let p = vec2<i32>(pos.xy);
    // Central filtering re-filters the current pixel; without it, a point sample.
    // A branch on the uniform, not a `select`, which would pay for both.
    var here: vec4<f32>;
    if (taa.levels.w >= 1.0) {
        here = filtered_center(p);
    } else {
        here = current(p);
    }
    let center = ycocg::from_rgb(here.rgb);

    // The neighbourhood box and its moments: the 3×3, or URP's 5-tap cross at
    // clamp quality 0.
    let cross = taa.levels.x < 1.0;
    var lo = center;
    var hi = center;
    var m1 = center;
    var m2 = center * center;
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            if ((x != 0 || y != 0) && !(cross && x != 0 && y != 0)) {
                let c = ycocg::from_rgb(current(p + vec2<i32>(x, y)).rgb);
                lo = min(lo, c);
                hi = max(hi, c);
                m1 += c;
                m2 += c * c;
            }
        }
    }
    // Variance clamping (URP's clamp quality 2 and up); below, the plain min/max box.
    if (taa.levels.x >= 2.0) {
        let mean = m1 / 9.0;
        let deviation = sqrt(abs(m2 / 9.0 - mean * mean));
        let spread = taa.knobs.y * deviation;
        lo = max(lo, mean - spread);
        hi = min(hi, mean + spread);
    }

    let uv = pos.xy * taa.size.zw;
    let front = frontmost(p);
    let frame = Frame(here, lo, hi, 1.0, reactivity(p, front.own.y));
    return Pixel(frame, uv, front);
}

// The reactivity around scene pixel `p`: the most reactive texel of its
// 3×3, as the motion is its 3×3's frontmost (ours: a translucent's edge the jitter
// moves by a pixel stays reactive), counting only translucents drawn by `camera`
// (the pixel's own surface's) or a camera above it, so a world window behind a
// viewmodel's gun leaves the gun alone. Zero where no translucent drew.
fn reactivity(p: vec2<i32>, camera: f32) -> f32 {
    // An opaque-only frame: the mask is all zero, nothing to read.
    if (taa.jitter.z < 0.5) {
        return 0.0;
    }
    let last = vec2<i32>(textureDimensions(t_reactive)) - 1;
    var r = 0.0;
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            let v = textureLoad(t_reactive, clamp(p + vec2<i32>(x, y), vec2<i32>(0), last), 0).r;
            let drawn_by = floor(v);
            if (v > 0.0 && drawn_by >= camera) {
                r = max(r, v - drawn_by);
            }
        }
    }
    return r;
}

// One pixel's current frame for [`blend`]: its colour, the box its history is
// clamped into (YCoCg), how much its weight is trusted (1, or TAAU's nearest
// sample's weight) and its reactivity ([`reactivity`]).
struct Frame {
    here: vec4<f32>,
    lo: vec3<f32>,
    hi: vec3<f32>,
    trust: f32,
    reactive: f32,
};

// How far a pixel moves (`.x`) and is a reveal (`.y`), each 0 to 1: moving
// as its motion goes from `knobs.w` to `rejection.y` history pixels, revealed as
// surface `now` is farther than last frame's around `prev_uv` ([`depth_mismatch`]).
fn change_of(motion: vec2<f32>, prev_uv: vec2<f32>, now: vec2<f32>) -> vec2<f32> {
    let pixels = length(motion * 0.5 * taa.size.xy);
    let moving = clamp((pixels - taa.knobs.w) / (taa.rejection.y - taa.knobs.w), 0.0, 1.0);
    let mismatch = depth_mismatch(prev_uv * taa.size.xy, now);
    let span = taa.rejection.w - taa.rejection.z;
    let revealed = clamp((mismatch - taa.rejection.z) / span, 0.0, 1.0);
    return vec2<f32>(moving, revealed);
}

// The frame's weight: the frame influence, raised toward a moving pixel's by
// `change.x`, times the frame's trust, then toward 1 as the pixel is a reveal,
// then toward 1 by the frame's reactivity (FSR2 scales the history's share by
// `1 - reactive`). A still, unrevealed pixel no translucent drew weighs exactly
// `knobs.x` × trust.
fn frame_weight(frame: Frame, change: vec2<f32>) -> f32 {
    let influence = mix(taa.knobs.x, taa.rejection.x, change.x) * frame.trust;
    let weight = mix(influence, 1.0, change.y);
    // Exactly the weight where no translucent drew.
    return select(weight, mix(weight, 1.0, frame.reactive), frame.reactive > 0.0);
}

// [`taa_flicker::Flicker`] at the pixel whose history is read at `prev_uv`:
// last frame's flicker texel there, the pixel's stillness and reactivity, its
// frame's perceptual `luma`, and its history's luma before (`raw`) and after its
// clamp (`boxed`).
fn flicker_of(prev_uv: vec2<f32>, luma: f32, raw: f32, boxed: f32, change: vec2<f32>, reactive: f32) -> taa_flicker::Flicker {
    let last = vec2<i32>(textureDimensions(t_flicker_history)) - 1;
    let at = clamp(vec2<i32>(floor(prev_uv * taa.size.xy)), vec2<i32>(0), last);
    let then = textureLoad(t_flicker_history, at, 0);
    let still = (1.0 - change.x) * (1.0 - change.y);
    return taa_flicker::next(then, luma, vec2<f32>(still, still * (1.0 - reactive)), vec2<f32>(raw, boxed), taa.flicker.xy);
}

// Last frame's resolve where a [`Pixel`]'s surface was (`prev_uv`, YCoCg `raw`),
// how far the pixel moves and is a reveal (`change`) and the frame's weight;
// `ok` false where there is no history to blend (none yet, off the image, or not
// finite).
struct Past {
    prev_uv: vec2<f32>,
    raw: vec3<f32>,
    change: vec2<f32>,
    weight: f32,
    ok: bool,
};

// The [`Past`] of `px`, whose surface moved by `px.front.motion` (`front`: its
// frontmost surface's NDC motion and that surface, and its own).
fn past_of(px: Pixel) -> Past {
    // NDC y points up, uv down.
    let prev_uv = px.uv - px.front.motion * vec2<f32>(0.5, -0.5);
    // Bicubic history (URP's history quality 2); one bilinear read below it. A
    // branch on the uniform, so each level pays for its own read only.
    var sampled: vec3<f32>;
    if (taa.levels.z >= 2.0) {
        sampled = history_bicubic(prev_uv);
    } else {
        sampled = textureSampleLevel(t_history, s_linear, prev_uv, 0.0).rgb;
    }
    let inside = all(abs(prev_uv - 0.5) <= vec2<f32>(0.5));
    if (taa.knobs.z < 0.5 || !inside || !numeric::finite3(sampled)) {
        return Past(prev_uv, vec3<f32>(0.0), vec2<f32>(0.0), 0.0, false);
    }
    let change = change_of(px.front.motion, prev_uv, px.front.near);
    let weight = frame_weight(px.frame, change);
    let raw = ycocg::from_rgb(max(sampled, vec3<f32>(0.0)));
    return Past(prev_uv, raw, change, weight, true);
}

// The colour of `frame` blended over its `history` (YCoCg, in its box) by
// `weight`, in URP's perceptual space: each colour over 1 + its luma, and back.
fn mixed(frame: Frame, history: vec3<f32>, weight: f32) -> vec4<f32> {
    let center = ycocg::from_rgb(frame.here.rgb);
    let a = history / (1.0 + history.x);
    let b = center / (1.0 + center.x);
    let m = mix(a, b, weight);
    let rgb = ycocg::to_rgb(m / max(1.0 - m.x, 1.0 / MAX_HDR));
    let out = clamp(rgb, vec3<f32>(0.0), vec3<f32>(MAX_HDR));
    return vec4<f32>(select(frame.here.rgb, out, numeric::finite3(rgb)), frame.here.a);
}

// URP's resolve of `px`: last frame's resolve where its surface was ([`past_of`]),
// clamped into the frame's box and lerped toward the frame by [`frame_weight`];
// the frame alone, exactly, with no history to blend.
fn blend(px: Pixel) -> ResolveOut {
    let past = past_of(px);
    if (!past.ok) {
        return ResolveOut(px.frame.here, px.front.own);
    }
    let history = into_box(past.raw, px.frame.lo, px.frame.hi);
    return ResolveOut(mixed(px.frame, history, past.weight), px.front.own);
}

// [`blend`] with the anti-flicker on: a blinking still pixel's history
// keeps its luma up to `flicker.y` times its recurring blinks outside the box,
// along the line to where it was ([`flicker_of`]); its hue and saturation stay the
// box's, so a hue change is clamped as URP's is.
fn blend_held(px: Pixel) -> HeldOut {
    let luma = ycocg::perceptual(ycocg::from_rgb(px.frame.here.rgb).x);
    let past = past_of(px);
    if (!past.ok) {
        return HeldOut(px.frame.here, px.front.own, taa_flicker::texel(luma, taa_flicker::NONE));
    }
    let boxed = into_box(past.raw, px.frame.lo, px.frame.hi);
    let f = flicker_of(past.prev_uv, luma, past.raw.x, boxed.x, past.change, px.frame.reactive);
    // The box's colour at the held luma: its chroma scales with it (YCoCg's chroma
    // is in light, not a ratio), so the hue and saturation stay the clamp's.
    let held = mix(boxed.x, past.raw.x, f.hold);
    let history = vec3<f32>(held, boxed.yz * select(0.0, held / boxed.x, boxed.x > 1e-6));
    let color = mixed(px.frame, history, past.weight);
    return HeldOut(color, px.front.own, taa_flicker::texel(luma, f));
}

// The history `h` brought into the box `lo`..`hi` (YCoCg): clamped per channel, or
// at clamp quality 3 clipped toward the box's centre (URP's `ClipToAABBCenter`,
// Playdead's), which keeps its hue where a clamp would shift it.
fn into_box(h: vec3<f32>, lo: vec3<f32>, hi: vec3<f32>) -> vec3<f32> {
    if (taa.levels.x < 3.0) {
        return clamp(h, lo, hi);
    }
    let center = 0.5 * (hi + lo);
    let extents = max(0.5 * (hi - lo), vec3<f32>(6.1e-5));
    let offset = h - center;
    let unit = abs(offset / extents);
    let most = max(unit.x, max(unit.y, unit.z));
    return select(h, center + offset / most, most > 1.0);
}

// The weight of a sample `d` scene pixels from where it should land: URP's
// `CalculateFilterWeights` (`TemporalAA.hlsl`), a Gaussian fit of a Blackman-Harris
// window of radius 1, positive everywhere, so the reconstruction never rings.
fn sample_weight(d: vec2<f32>) -> f32 {
    return exp(-2.29 * dot(d, d));
}

// TAAU: output pixel `pos` of a scaled view, from the scene-size `t_current` (the
// scene, or depth of field's result) and the output-size history. The 3x3 scene pixels around the output
// pixel's centre each sampled the unjittered image `jitter` away from their own
// centre (the projection moved the image by `jitter`, y flipped into pixel rows);
// each is weighted by where its sample landed, the history is clamped into their
// box as `fs_resolve` does, and the frame's weight is trusted as far as its
// nearest sample's weight, so a sample far from this pixel only nudges it.
@fragment
fn fs_upscale(@builtin(position) pos: vec4<f32>) -> ResolveOut {
    return blend(upscaled(pos));
}

// `fs_upscale` with the camera's anti-flicker on.
@fragment
fn fs_upscale_held(@builtin(position) pos: vec4<f32>) -> HeldOut {
    return blend_held(upscaled(pos));
}

// The upscaling resolve's [`Pixel`] at output pixel `pos`.
fn upscaled(pos: vec4<f32>) -> Pixel {
    let uv = pos.xy * taa.size.zw;
    let at = uv * taa.scene.xy;
    let base = vec2<i32>(floor(at));
    let shift = vec2<f32>(taa.jitter.x, -taa.jitter.y);

    var lo = vec3<f32>(1e9);
    var hi = vec3<f32>(-1e9);
    var m1 = vec3<f32>(0.0);
    var m2 = vec3<f32>(0.0);
    var sum = vec3<f32>(0.0);
    var weights = 0.0;
    var nearest = 0.0;
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            let k = base + vec2<i32>(x, y);
            let c = current(k).rgb;
            let w = sample_weight(vec2<f32>(k) + 0.5 - shift - at);
            sum += c * w;
            weights += w;
            nearest = max(nearest, w);
            let v = ycocg::from_rgb(c);
            lo = min(lo, v);
            hi = max(hi, v);
            m1 += v;
            m2 += v * v;
        }
    }
    let here = vec4<f32>(sum / max(weights, 1e-6), current(base).a);
    // Variance clamping (URP's clamp quality 2 and up); lower levels keep the
    // min/max box, of the 3×3 the reconstruction reads at every level.
    if (taa.levels.x >= 2.0) {
        let mean = m1 / 9.0;
        let spread = taa.knobs.y * sqrt(abs(m2 / 9.0 - mean * mean));
        lo = max(lo, mean - spread);
        hi = min(hi, mean + spread);
    }
    let front = frontmost(base);
    let frame = Frame(here, lo, hi, nearest, reactivity(base, front.own.y));
    return Pixel(frame, uv, front);
}
