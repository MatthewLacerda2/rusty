// engine/shaders/forward/detail.wgsl — a material's detail layer (#1997), Unity
// HDRP Lit's *Detail Map*: one packed texture (R the desaturated albedo, G the
// normal's Y, B the smoothness, A the normal's X; 0.5 neutral) read at a tiling of
// its own, changing the albedo, the roughness and the shading normal before any
// light is summed, so specular and reflections are never tinted by it.
//
// The albedo and smoothness follow HDRP's `LitDataIndividualLayer.hlsl`: each
// channel's distance from 0.5, times its scale, pushes the value toward 0 or 1
// (the albedo in HDRP's square-root space). The normal is HDRP's AG unpack, scaled;
// where HDRP blends it onto the main tangent-space normal (RNM), rusty reorients it
// onto the main shading normal in world space: by the mesh's tangent frame
// re-orthogonalised against it under UV0, by the whiteout blend per projection
// under a world mapping, as the main normal map does (`mapping::sample_normal`).
// HDRP's *Detail Mask* (#2258) is a texture of its own (rusty has no mask map):
// its red, read at the main maps' coordinates as HDRP reads it with the main UVs
// (the height map's parallax shift included), blends each detailed result (albedo,
// smoothness, normal) back toward the surface's own value per texel, as HDRP's
// `lerp(base, detailed, detailMask)`.
//
// The layer shares the material's mapping. Under UV0 its coordinates are the main
// maps' tiled UV under its own scale/offset (HDRP's *Link Details with Base*);
// under Planar and Triplanar the world projection at its own repeats per metre.
// Every branch is on `material()`, so the samples sit in uniform control flow.

#define_import_path forward::detail

#import forward::bindings::{material, s_detail, s_detail_mask, t_detail, t_detail_mask}
#import forward::mapping::{MapCoords, Mapping, coords_with, displaced, lookup, lookup_uv, sample, whiteout}

const UV0: u32 = 0u;
const TRIPLANAR: u32 = 2u;

// The surface inputs the layer changes.
struct Detailed {
    albedo: vec3<f32>,
    roughness: f32,
    normal: vec3<f32>,
};

// Where the detail is read at a point whose main coordinates are `main` (world
// `position`, geometric world `normal`): `mapping::coords_with` under the layer's
// terms, its UV the main maps' tiled one. Pure maths, no sample.
fn coords(main: MapCoords, position: vec3<f32>, normal: vec3<f32>) -> MapCoords {
    let m = material();
    let terms = Mapping(m.detail_st, m.uv_mapping, m.detail_world_scale, m.blend_sharpness, 0u);
    var c = coords_with(terms, main.uv, position, normal);
    if (!displaced()) {
        return c;
    }
    // The height map's parallax (#2191, UV0 or Planar): the main maps' shift and
    // derivatives in the detail's repeats. Under UV0 its UV is the shifted main one
    // already; under Planar it is projected afresh from the world, so it takes the
    // shift here, scaled from the main repeats to its own.
    var ratio = m.detail_st.xy;
    if (m.uv_mapping != UV0) {
        let main_repeats = m.uv_st.xy * m.world_scale;
        let safe = select(main_repeats, vec2<f32>(1.0), abs(main_repeats) < vec2<f32>(1e-6));
        ratio = m.detail_st.xy * m.detail_world_scale / safe;
        c.uv += main.shift * ratio;
    }
    c.x = c.uv;
    c.y = c.uv;
    c.z = c.uv;
    c.shift = main.shift * ratio;
    c.dx = main.dx * ratio;
    c.dy = main.dy * ratio;
    c.bend = main.bend;
    return c;
}

// How much of the detail applies at the point whose main coordinates are `main`:
// the mask's red, under every mapping the main maps use; 1 without a mask (or a
// detail map), which is then never fetched.
fn mask(main: MapCoords) -> f32 {
    let m = material();
    if (m.use_detail_map == 0u || m.use_detail_mask == 0u) {
        return 1.0;
    }
    return saturate(sample(t_detail_mask, s_detail_mask, main).r);
}

// HDRP's detail normal from a texel: x from alpha, y from green, z rebuilt, then x
// and y scaled (`UnpackNormalAG`).
fn unpack(texel: vec4<f32>, scale: f32) -> vec3<f32> {
    let xy = texel.ag * 2.0 - 1.0;
    let z = sqrt(max(1.0 - saturate(dot(xy, xy)), 0.0));
    return normalize(vec3<f32>(xy * scale, z));
}

// `value` pushed toward 0 (`d` below 0) or 1 by `amount` in [0, 1].
fn toward(value: f32, d: f32, amount: f32) -> f32 {
    return mix(value, select(1.0, 0.0, d < 0.0), amount);
}

// The albedo, roughness and normal `in_*` with the detail at `c` applied, each
// lerped back toward its input by `1 - amount` (HDRP's `detailMask`, from
// `mask`), or as given when the material has
// no detail map; the roughness kept in [0.04, 1], the
// range `fs_main` shades with. `tangent` is the mesh's (`w` its
// handedness), read under UV0 only.
fn apply(c: MapCoords, amount: f32, in_albedo: vec3<f32>, in_roughness: f32, n: vec3<f32>, tangent: vec4<f32>) -> Detailed {
    let m = material();
    if (m.use_detail_map == 0u) {
        return Detailed(in_albedo, in_roughness, n);
    }
    // One fetch per projection serves all four channels; UV0 and Planar read one.
    let ty = lookup_uv(t_detail, s_detail, c);
    var tx = ty;
    var tz = ty;
    var w = vec3<f32>(0.0, 1.0, 0.0);
    if (m.uv_mapping == TRIPLANAR) {
        tx = lookup(t_detail, s_detail, c.x);
        tz = lookup(t_detail, s_detail, c.z);
        w = c.weights;
    }
    let texel = tx * w.x + ty * w.y + tz * w.z;

    // Albedo (HDRP): toward black or white in square-root space, by the square
    // of the channel's push.
    let d_albedo = texel.r * 2.0 - 1.0;
    let push = saturate(abs(d_albedo) * m.detail_albedo_scale);
    let end = select(vec3<f32>(1.0), vec3<f32>(0.0), d_albedo < 0.0);
    let overlay = mix(sqrt(max(in_albedo, vec3<f32>(0.0))), end, push * push);
    // HDRP's `detailMask` lerps the result back toward the base colour.
    let albedo = mix(in_albedo, saturate(overlay * overlay), amount);

    // Smoothness (HDRP): toward 0 or 1, linearly.
    let d_smooth = texel.b * 2.0 - 1.0;
    let smooth_push = saturate(abs(d_smooth) * m.detail_smoothness_scale) * amount;
    let roughness = clamp(1.0 - toward(1.0 - in_roughness, d_smooth, smooth_push), 0.04, 1.0);

    // Normal: onto the main shading normal `n`.
    let scale = m.detail_normal_scale;
    var normal: vec3<f32>;
    if (m.uv_mapping == UV0) {
        let along = tangent.xyz - n * dot(n, tangent.xyz);
        let t = normalize(select(along, vec3<f32>(1.0, 0.0, 0.0), dot(along, along) < 1e-12));
        let b = cross(n, t) * tangent.w;
        normal = normalize(mat3x3<f32>(t, b, n) * unpack(ty, scale));
    } else {
        normal = whiteout(unpack(tx, scale), unpack(ty, scale), unpack(tz, scale), w, n);
    }
    // HDRP lerps the blended normal back toward the main one by the mask.
    return Detailed(albedo, roughness, normalize(mix(n, normal, amount)));
}
