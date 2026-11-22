// Two `Rgba16Float` SSR texels per pixel: on a hit, `postfx/ssr.wgsl` swaps `applied`, the
// environment reflection the forward pass added, for `weight` × the hit; zero gets no SSR. Each
// alpha holds an integer (`pack_lane`), so neither texel may be blended or filtered.

#define_import_path lighting::ssr_surface

// An octahedral component's levels (even, so the middle one is exactly 0) and the roughness
// digits' base.
const OCT_LEVELS: f32 = 2046.0;
const DIGIT: u32 = 15u;
// The payload's offset into the `f16` bits: exponent 1, the smallest normal number.
const F16_NORMAL: u32 = 1024u;

// An octahedral component `e` in [-1, 1] and a roughness digit (0..14), as the normal `f16`
// whose bits are the payload, built exactly in `f32` (`pack2x16float` needs a capability naga
// does not grant).
fn pack_lane(e: f32, digit: u32) -> f32 {
    let q = u32(round(clamp(e * 0.5 + 0.5, 0.0, 1.0) * OCT_LEVELS));
    let bits = q * DIGIT + digit + F16_NORMAL;
    let mantissa = 1.0 + f32(bits & 1023u) / 1024.0;
    return ldexp(mantissa, i32(bits >> 10u) - 15);
}

fn unpack_lane(a: f32) -> vec2<f32> {
    let split = frexp(max(a, ldexp(1.0, -14)));
    let exponent = u32(split.exp + 14);
    let mantissa = u32(round((split.fract * 2.0 - 1.0) * 1024.0));
    let payload = ((exponent << 10u) | mantissa) - F16_NORMAL;
    // Integer division: Metal's fast math may divide by a reciprocal, so a float difference of 0
    // could land at about -1e-9.
    let level = i32(payload / DIGIT) * 2 - i32(OCT_LEVELS);
    return vec2<f32>(f32(level) / OCT_LEVELS, f32(payload % DIGIT));
}

// Octahedral encoding of a unit vector into [-1, 1]² (Cigolle et al. 2014).
fn oct_encode(n: vec3<f32>) -> vec2<f32> {
    let p = n.xy / (abs(n.x) + abs(n.y) + abs(n.z));
    if (n.z >= 0.0) {
        return p;
    }
    return (1.0 - abs(p.yx)) * select(vec2<f32>(-1.0), vec2<f32>(1.0), p >= vec2<f32>(0.0));
}

fn oct_decode(e: vec2<f32>) -> vec3<f32> {
    var n = vec3<f32>(e, 1.0 - abs(e.x) - abs(e.y));
    if (n.z < 0.0) {
        let folded = (1.0 - abs(n.yx)) * select(vec2<f32>(-1.0), vec2<f32>(1.0), n.xy >= vec2<f32>(0.0));
        n = vec3<f32>(folded, n.z);
    }
    return normalize(n);
}

struct SsrTexels {
    weight: vec4<f32>,
    applied: vec4<f32>,
};

// How much SSR a surface of `roughness` gets under *Minimum Smoothness* `min_smoothness`: 0
// below it, 1 from `width` of smoothness above it (HDRP's *Smoothness Fade Start*).
fn smoothness_fade(roughness: f32, min_smoothness: f32, width: f32) -> f32 {
    return clamp((1.0 - roughness - min_smoothness) / max(width, 1e-4), 0.0, 1.0);
}

// The texels of a surface with shading normal `n` and `roughness`, whose environment reflection
// `applied` was added under `weight` (both after `ao`), then fogged by fog factor `fog`.
fn encode(n: vec3<f32>, roughness: f32, applied: vec3<f32>, weight: vec3<f32>, fog: f32) -> SsrTexels {
    let k = 1.0 - fog;
    let e = oct_encode(n);
    let q = u32(round(clamp(roughness, 0.0, 1.0) * f32(DIGIT * DIGIT - 1u)));
    let a = vec2<f32>(pack_lane(e.x, q / DIGIT), pack_lane(e.y, q % DIGIT));
    return SsrTexels(vec4<f32>(weight * k, a.x), vec4<f32>(applied * k, a.y));
}

fn scale(t: SsrTexels, k: vec3<f32>) -> SsrTexels {
    let f = max(k, vec3<f32>(0.0));
    return SsrTexels(vec4<f32>(t.weight.rgb * f, t.weight.a), vec4<f32>(t.applied.rgb * f, t.applied.a));
}

// A decoded pair: `roughness` is perceptual, to 1/224.
struct SsrSurface {
    normal: vec3<f32>,
    roughness: f32,
    weight: vec3<f32>,
    applied: vec3<f32>,
};

fn decode(weight: vec4<f32>, applied: vec4<f32>) -> SsrSurface {
    let x = unpack_lane(weight.w);
    let y = unpack_lane(applied.w);
    let roughness = (x.y * f32(DIGIT) + y.y) / f32(DIGIT * DIGIT - 1u);
    return SsrSurface(oct_decode(vec2<f32>(x.x, y.x)), roughness, weight.rgb, applied.rgb);
}

fn reflects(s: SsrSurface) -> bool {
    return any(s.weight > vec3<f32>(0.0));
}
