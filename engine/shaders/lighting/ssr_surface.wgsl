// engine/shaders/lighting/ssr_surface.wgsl — the SSR surface target (#1526): what the
// forward pass of a view running screen-space reflections writes beside its velocity
// target, and what `postfx/effects/ssr.wgsl` reads back. One `Rgba16Float` texel per
// pixel:
//   rg — the shading normal (normal-mapped, decal-perturbed), octahedron-encoded
//   b  — the perceptual smoothness, `1 - roughness`
//   a  — how much of the environment reflection the forward pass gave this pixel (the
//        luminance of its Fresnel × reflection scale, step 5 of `shader.wgsl`):
//        positive when that reflection was the sky, negative when a reflection probe
//        lit it (#1206), so SSR knows what its hit replaces
// A texel nothing wrote (the clear, the sky, an unlit surface, the black texel a view
// without the target binds) is zero: no smoothness, so no SSR.

#define_import_path lighting::ssr_surface

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

// The texel for a surface with shading normal `n`, `roughness`, and an environment
// reflection weighted by `weight` (a colour) that a reflection probe gave it when
// `probe_lit`.
fn encode(n: vec3<f32>, roughness: f32, weight: vec3<f32>, probe_lit: bool) -> vec4<f32> {
    let w = dot(weight, vec3<f32>(0.2126, 0.7152, 0.0722));
    return vec4<f32>(oct_encode(n), 1.0 - roughness, select(w, -w, probe_lit));
}

// A decoded texel.
struct SsrSurface {
    normal: vec3<f32>,
    smoothness: f32,
    // The environment reflection's weight, always positive.
    weight: f32,
    probe_lit: bool,
};

fn decode(t: vec4<f32>) -> SsrSurface {
    return SsrSurface(oct_decode(t.xy), t.z, abs(t.w), t.w < 0.0);
}
