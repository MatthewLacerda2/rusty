// URP's `TemporalAA.hlsl` at *High* (`DoTemporalAA(2, 2, 2, 0)`): variance-clamped 3×3
// YCoCg box; the motion of the 3×3's frontmost surface (so an edge reprojects with what
// is in front: a viewmodel's over the world's); a 5-tap bicubic history; blended in
// URP's perceptual space (colour over 1 + luma) so a bright highlight cannot dominate.
// Beyond URP: the frame weighs more as the pixel moves or a translucent drew (its
// reactive mask, FSR2's), and the history fades on a reveal. The `_held` entry points
// run HDRP's anti-flicker (`taa_flicker.wgsl`). Velocity target: `.xy` is `ndc_now -
// ndc_prev` unjittered, `.z` the view depth (sky: 65504), `.w` the drawing camera's
// index in its stack: the frontmost surface is the highest `.w`, then the smallest
// `.z`.

#import fullscreen
#import numeric
#import postfx::taa::io::{taa, t_current, t_history, s_linear, t_reactive, t_flicker_history, current, MAX_HDR}
#import postfx::taa::surface::{Front, frontmost, depth_mismatch}
#import postfx::taa_flicker
#import postfx::ycocg

struct ResolveOut {
    @location(0) color: vec4<f32>,
    @location(1) surface: vec2<f32>,
};

struct HeldOut {
    @location(0) color: vec4<f32>,
    @location(1) surface: vec2<f32>,
    @location(2) flicker: vec4<f32>,
};

@vertex
fn vs_fullscreen(@builtin(vertex_index) vid: u32) -> @builtin(position) vec4<f32> {
    return fullscreen::triangle(vid);
}

// Jimenez's *Filmic SMAA*, as URP's `SampleBicubic5TapHalf`.
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

struct Pixel {
    frame: Frame,
    uv: vec2<f32>,
    front: Front,
};

@fragment
fn fs_resolve(@builtin(position) pos: vec4<f32>) -> ResolveOut {
    return blend(resolved(pos));
}

@fragment
fn fs_resolve_held(@builtin(position) pos: vec4<f32>) -> HeldOut {
    return blend_held(resolved(pos));
}

fn resolved(pos: vec4<f32>) -> Pixel {
    let p = vec2<i32>(pos.xy);
    var here: vec4<f32>;
    if (taa.levels.w >= 1.0) {
        here = filtered_center(p);
    } else {
        here = current(p);
    }
    let center = ycocg::from_rgb(here.rgb);

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

fn reactivity(p: vec2<i32>, camera: f32) -> f32 {
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

struct Frame {
    here: vec4<f32>,
    lo: vec3<f32>,
    hi: vec3<f32>,
    trust: f32,
    reactive: f32,
};

fn change_of(motion: vec2<f32>, prev_uv: vec2<f32>, now: vec2<f32>) -> vec2<f32> {
    let pixels = length(motion * 0.5 * taa.size.xy);
    let moving = clamp((pixels - taa.knobs.w) / (taa.rejection.y - taa.knobs.w), 0.0, 1.0);
    let mismatch = depth_mismatch(prev_uv * taa.size.xy, now);
    let span = taa.rejection.w - taa.rejection.z;
    let revealed = clamp((mismatch - taa.rejection.z) / span, 0.0, 1.0);
    return vec2<f32>(moving, revealed);
}

// FSR2 scales the history's share by `1 - reactive`.
fn frame_weight(frame: Frame, change: vec2<f32>) -> f32 {
    let influence = mix(taa.knobs.x, taa.rejection.x, change.x) * frame.trust;
    let weight = mix(influence, 1.0, change.y);
    return select(weight, mix(weight, 1.0, frame.reactive), frame.reactive > 0.0);
}

fn flicker_of(prev_uv: vec2<f32>, luma: f32, raw: f32, boxed: f32, change: vec2<f32>, reactive: f32) -> taa_flicker::Flicker {
    let last = vec2<i32>(textureDimensions(t_flicker_history)) - 1;
    let at = clamp(vec2<i32>(floor(prev_uv * taa.size.xy)), vec2<i32>(0), last);
    let then = textureLoad(t_flicker_history, at, 0);
    let still = (1.0 - change.x) * (1.0 - change.y);
    return taa_flicker::next(then, luma, vec2<f32>(still, still * (1.0 - reactive)), vec2<f32>(raw, boxed), taa.flicker.xy);
}

struct Past {
    prev_uv: vec2<f32>,
    raw: vec3<f32>,
    change: vec2<f32>,
    weight: f32,
    ok: bool,
};

fn past_of(px: Pixel) -> Past {
    // NDC y points up, uv down.
    let prev_uv = px.uv - px.front.motion * vec2<f32>(0.5, -0.5);
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

fn mixed(frame: Frame, history: vec3<f32>, weight: f32) -> vec4<f32> {
    let center = ycocg::from_rgb(frame.here.rgb);
    let a = history / (1.0 + history.x);
    let b = center / (1.0 + center.x);
    let m = mix(a, b, weight);
    let rgb = ycocg::to_rgb(m / max(1.0 - m.x, 1.0 / MAX_HDR));
    let out = clamp(rgb, vec3<f32>(0.0), vec3<f32>(MAX_HDR));
    return vec4<f32>(select(frame.here.rgb, out, numeric::finite3(rgb)), frame.here.a);
}

fn blend(px: Pixel) -> ResolveOut {
    let past = past_of(px);
    if (!past.ok) {
        return ResolveOut(px.frame.here, px.front.own);
    }
    let history = into_box(past.raw, px.frame.lo, px.frame.hi);
    return ResolveOut(mixed(px.frame, history, past.weight), px.front.own);
}

fn blend_held(px: Pixel) -> HeldOut {
    let luma = ycocg::perceptual(ycocg::from_rgb(px.frame.here.rgb).x);
    let past = past_of(px);
    if (!past.ok) {
        return HeldOut(px.frame.here, px.front.own, taa_flicker::texel(luma, taa_flicker::NONE));
    }
    let boxed = into_box(past.raw, px.frame.lo, px.frame.hi);
    let f = flicker_of(past.prev_uv, luma, past.raw.x, boxed.x, past.change, px.frame.reactive);
    let held = mix(boxed.x, past.raw.x, f.hold);
    let history = vec3<f32>(held, boxed.yz * select(0.0, held / boxed.x, boxed.x > 1e-6));
    let color = mixed(px.frame, history, past.weight);
    return HeldOut(color, px.front.own, taa_flicker::texel(luma, f));
}

// URP's `ClipToAABBCenter` (Playdead's) keeps hue where a clamp would shift it.
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

// URP's `CalculateFilterWeights`: a Gaussian fit of a Blackman-Harris window of radius
// 1, positive everywhere, so the reconstruction never rings.
fn sample_weight(d: vec2<f32>) -> f32 {
    return exp(-2.29 * dot(d, d));
}

// The projection moved the image by `jitter`, y flipped into pixel rows.
@fragment
fn fs_upscale(@builtin(position) pos: vec4<f32>) -> ResolveOut {
    return blend(upscaled(pos));
}

@fragment
fn fs_upscale_held(@builtin(position) pos: vec4<f32>) -> HeldOut {
    return blend_held(upscaled(pos));
}

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
