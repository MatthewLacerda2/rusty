// engine/shaders/lighting/ssr_surface.wgsl — the SSR surface targets (#1526): what the
// forward pass of a view running screen-space reflections writes beside its velocity
// target, and what `postfx/ssr.wgsl` reads back. Two `Rgba16Float` texels per
// pixel, so an SSR hit replaces exactly the environment reflection the forward pass
// applied (HDRP's hierarchy: SSR, then a reflection probe, then the sky):
//   weight  — rgb: what a hit's colour is multiplied by: the Fresnel colour × reflection
//             scale × `ao` × (1 − fog); a: the shading normal's octahedral x and the
//             roughness's high digit, packed (`pack_lane`)
//   applied — rgb: the environment reflection (probe or sky alike) step 5 of
//             `shader.wgsl` added, after `ao` and fog; a: the normal's octahedral y
//             and the roughness's low digit, packed
// The SSR pass fades both by the volume's *Minimum Smoothness* (#1736, `smoothness_fade`)
// and picks the blur from the roughness, so the forward pass needs no SSR setting.
// The composite does `pixel − applied + weight × hit` on a hit and leaves a miss alone.
// A texel nothing wrote (the clear, the sky, an unlit surface, the black texel a view
// without the targets binds) is zero: no weight, so no SSR.
//
// The packing (#1736): the eight halves are full, and a ninth target would pass
// wgpu's 32 bytes of colour attachments per sample. So each alpha carries an integer
// payload in the bits of an `f16` (exponent 1..30, never a denormal, an infinity or
// a NaN, so the write and read are exact): an 11-bit octahedral component (about
// what an `f16` in [-1, 1] held) times 15, plus a base-15 digit of the roughness,
// which so takes 225 levels.

#define_import_path lighting::ssr_surface

// How much smoothness above the *Minimum Smoothness* SSR takes to fade fully in:
// HDRP's *Smoothness Fade Start* is the minimum plus this (`Ssr::SMOOTHNESS_FADE`).
const SMOOTHNESS_FADE: f32 = 0.2;
// The packing's levels: of an octahedral component, and of each roughness digit.
const OCT_LEVELS: f32 = 2047.0;
const DIGIT: u32 = 15u;
// The payload's offset into the `f16` bits: exponent 1, the smallest normal number.
const F16_NORMAL: u32 = 1024u;

// An octahedral component `e` in [-1, 1] and a roughness digit (0..14), as the `f16`
// value an alpha holds: the payload's bits read as a normal `f16`, `(1 + m/1024) ×
// 2^(e − 15)`, built exactly in `f32` (`ldexp`; `pack2x16float` needs a capability
// naga does not grant).
fn pack_lane(e: f32, digit: u32) -> f32 {
    let q = u32(round(clamp(e * 0.5 + 0.5, 0.0, 1.0) * OCT_LEVELS));
    let bits = q * DIGIT + digit + F16_NORMAL;
    let mantissa = 1.0 + f32(bits & 1023u) / 1024.0;
    return ldexp(mantissa, i32(bits >> 10u) - 15);
}

// `pack_lane`'s two parts, back: x the octahedral component, y the digit. `frexp`
// splits the value into a fraction in [0.5, 1) and its exponent, exactly.
fn unpack_lane(a: f32) -> vec2<f32> {
    let split = frexp(max(a, ldexp(1.0, -14)));
    let exponent = u32(split.exp + 14);
    let mantissa = u32(round((split.fract * 2.0 - 1.0) * 1024.0));
    let payload = ((exponent << 10u) | mantissa) - F16_NORMAL;
    return vec2<f32>(f32(payload / DIGIT) / OCT_LEVELS * 2.0 - 1.0, f32(payload % DIGIT));
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

// The two texels, as the forward pass writes them.
struct SsrTexels {
    weight: vec4<f32>,
    applied: vec4<f32>,
};

// How much SSR a surface of `roughness` gets under *Minimum Smoothness* `min_smoothness`:
// 0 below it, 1 from *Smoothness Fade Start* (`SMOOTHNESS_FADE` above it).
fn smoothness_fade(roughness: f32, min_smoothness: f32) -> f32 {
    return clamp((1.0 - roughness - min_smoothness) / SMOOTHNESS_FADE, 0.0, 1.0);
}

// The texels of a surface with shading normal `n` and `roughness`, whose environment
// reflection `applied` was added under `weight` (both after `ao`), then fogged by `fog`
// (the fog factor, 0: none).
fn encode(n: vec3<f32>, roughness: f32, applied: vec3<f32>, weight: vec3<f32>, fog: f32) -> SsrTexels {
    let k = 1.0 - fog;
    let e = oct_encode(n);
    let q = u32(round(clamp(roughness, 0.0, 1.0) * f32(DIGIT * DIGIT - 1u)));
    let a = vec2<f32>(pack_lane(e.x, q / DIGIT), pack_lane(e.y, q % DIGIT));
    return SsrTexels(vec4<f32>(weight * k, a.x), vec4<f32>(applied * k, a.y));
}

// The texels of a surface whose colour an authored block then scaled by `k` per
// channel (`shadergen::blocks::Reflection`): the reflection in it and the weight a
// hit gets scale alike, so SSR replaces what the colour still holds. `k` = 0: no SSR.
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

// Whether a decoded surface can get SSR: a lit one (the pass then fades it by its
// smoothness).
fn reflects(s: SsrSurface) -> bool {
    return any(s.weight > vec3<f32>(0.0));
}
