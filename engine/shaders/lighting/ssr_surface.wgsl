// engine/shaders/lighting/ssr_surface.wgsl — the SSR surface targets (#1526): what the
// forward pass of a view running screen-space reflections writes beside its velocity
// target, and what `postfx/ssr.wgsl` reads back. Two `Rgba16Float` texels per
// pixel, so an SSR hit replaces exactly the environment reflection the forward pass
// applied (HDRP's hierarchy: SSR, then a reflection probe, then the sky):
//   weight  — rgb: what a hit's colour is multiplied by: the Fresnel colour × reflection
//             scale × `ao` × (1 − fog), faded by smoothness; a: the shading normal's
//             octahedral x
//   applied — rgb: the environment reflection (probe or sky alike) step 5 of
//             `shader.wgsl` added, after `ao` and fog, faded alike; a: the normal's
//             octahedral y
// The composite does `pixel − applied + weight × hit` on a hit and leaves a miss alone.
// A texel nothing wrote (the clear, the sky, an unlit surface, the black texel a view
// without the targets binds) is zero: no weight, so no SSR.

#define_import_path lighting::ssr_surface

// Below this smoothness a surface gets no SSR; above `SMOOTHNESS_FADE_START` it gets
// all of it (HDRP's *Minimum Smoothness* and *Smoothness Fade Start*).
const MIN_SMOOTHNESS: f32 = 0.8;
const SMOOTHNESS_FADE_START: f32 = 0.9;

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

// How much SSR a surface of `roughness` gets: 0 below *Minimum Smoothness*, 1 above
// *Smoothness Fade Start*.
fn smoothness_fade(roughness: f32) -> f32 {
    return clamp((1.0 - roughness - MIN_SMOOTHNESS) / (SMOOTHNESS_FADE_START - MIN_SMOOTHNESS), 0.0, 1.0);
}

// The texels of a surface with shading normal `n` and `roughness`, whose environment
// reflection `applied` was added under `weight` (both after `ao`), then fogged by `fog`
// (the fog factor, 0: none).
fn encode(n: vec3<f32>, roughness: f32, applied: vec3<f32>, weight: vec3<f32>, fog: f32) -> SsrTexels {
    let k = smoothness_fade(roughness) * (1.0 - fog);
    let e = oct_encode(n);
    return SsrTexels(vec4<f32>(weight * k, e.x), vec4<f32>(applied * k, e.y));
}

// The texels of a surface whose colour an authored block then scaled by `k` per
// channel (`shadergen::blocks::Reflection`): the reflection in it and the weight a
// hit gets scale alike, so SSR replaces what the colour still holds. `k` = 0: no SSR.
fn scale(t: SsrTexels, k: vec3<f32>) -> SsrTexels {
    let f = max(k, vec3<f32>(0.0));
    return SsrTexels(vec4<f32>(t.weight.rgb * f, t.weight.a), vec4<f32>(t.applied.rgb * f, t.applied.a));
}

// A decoded pair.
struct SsrSurface {
    normal: vec3<f32>,
    weight: vec3<f32>,
    applied: vec3<f32>,
};

fn decode(weight: vec4<f32>, applied: vec4<f32>) -> SsrSurface {
    return SsrSurface(oct_decode(vec2<f32>(weight.w, applied.w)), weight.rgb, applied.rgb);
}

// Whether a decoded surface gets any SSR.
fn reflects(s: SsrSurface) -> bool {
    return any(s.weight > vec3<f32>(0.0));
}
