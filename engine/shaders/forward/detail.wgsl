// Unity HDRP Lit's *Detail Map*: one packed texture (R desaturated albedo, G normal Y,
// B smoothness, A normal X; 0.5 neutral). Albedo and smoothness follow HDRP's
// `LitDataIndividualLayer.hlsl` (albedo in square-root space). The normal is HDRP's AG
// unpack, reoriented onto the main shading normal in world space where HDRP blends with
// RNM.

#define_import_path forward::detail

#import forward::bindings::{material, s_detail, s_detail_mask, t_detail, t_detail_mask}
#import forward::mapping::{MapCoords, Mapping, coords_with, displaced, lookup, lookup_uv, sample, whiteout}

const UV0: u32 = 0u;
const TRIPLANAR: u32 = 2u;

struct Detailed {
    albedo: vec3<f32>,
    roughness: f32,
    normal: vec3<f32>,
};

fn coords(main: MapCoords, position: vec3<f32>, normal: vec3<f32>) -> MapCoords {
    let m = material();
    let terms = Mapping(m.detail_st, m.uv_mapping, m.detail_world_scale, m.blend_sharpness, 0u);
    var c = coords_with(terms, main.uv, position, normal);
    if (!displaced()) {
        return c;
    }
    // Under UV0 its UV is the shifted main one already; under Planar it is projected
    // afresh, so it takes the shift here.
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

// 1 without a mask (or a detail map), which is then never fetched.
fn mask(main: MapCoords) -> f32 {
    let m = material();
    if (m.use_detail_map == 0u || m.use_detail_mask == 0u) {
        return 1.0;
    }
    return saturate(sample(t_detail_mask, s_detail_mask, main).r);
}

fn unpack(texel: vec4<f32>, scale: f32) -> vec3<f32> {
    let xy = texel.ag * 2.0 - 1.0;
    let z = sqrt(max(1.0 - saturate(dot(xy, xy)), 0.0));
    return normalize(vec3<f32>(xy * scale, z));
}

fn toward(value: f32, d: f32, amount: f32) -> f32 {
    return mix(value, select(1.0, 0.0, d < 0.0), amount);
}

// The roughness is kept in [0.04, 1], the range `fs_main` shades with. `tangent.w` is
// handedness, read under UV0 only.
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

    // Albedo (HDRP): toward black or white in square-root space.
    let d_albedo = texel.r * 2.0 - 1.0;
    let push = saturate(abs(d_albedo) * m.detail_albedo_scale);
    let end = select(vec3<f32>(1.0), vec3<f32>(0.0), d_albedo < 0.0);
    let overlay = mix(sqrt(max(in_albedo, vec3<f32>(0.0))), end, push * push);
    let albedo = mix(in_albedo, saturate(overlay * overlay), amount);

    let d_smooth = texel.b * 2.0 - 1.0;
    let smooth_push = saturate(abs(d_smooth) * m.detail_smoothness_scale) * amount;
    let roughness = clamp(1.0 - toward(1.0 - in_roughness, d_smooth, smooth_push), 0.04, 1.0);

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
    return Detailed(albedo, roughness, normalize(mix(n, normal, amount)));
}
