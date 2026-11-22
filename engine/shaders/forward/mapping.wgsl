// where a surface reads its material maps : Unity Lit's Tiling/Offset on UV0, or HDRP Lit's
// Planar and Triplanar *Base UV Mapping*, projected from world space so a scaled primitive
// keeps one texel density on every face.

#define_import_path forward::mapping

#import forward::bindings::{camera, material, s_occlusion, s_specular, t_occlusion, t_specular}
#import forward::maps::{map_factor, unpack_normal}

const UV0: u32 = 0u;
const TRIPLANAR: u32 = 2u;

struct MapCoords {
    uv: vec2<f32>,
    x: vec2<f32>,
    y: vec2<f32>,
    z: vec2<f32>,
    weights: vec3<f32>,
    shift: vec2<f32>,
    dx: vec2<f32>,
    dy: vec2<f32>,
    bend: f32,
};

fn displaced() -> bool {
    let m = material();
    return m.use_height_map != 0u && m.height_scale > 0.0;
}

// A material's mapping terms: `render::gpu::uniforms::MappingUniform` byte-for-byte.
struct Mapping {
    uv_st: vec4<f32>,
    uv_mapping: u32,
    world_scale: f32,
    blend_sharpness: f32,
    _pad: u32,
};

fn material_mapping() -> Mapping {
    let m = material();
    return Mapping(m.uv_st, m.uv_mapping, m.world_scale, m.blend_sharpness, 0u);
}

fn tile_with(m: Mapping, uv: vec2<f32>) -> vec2<f32> {
    return uv * m.uv_st.xy + m.uv_st.zw;
}

fn coords(uv: vec2<f32>, position: vec3<f32>, normal: vec3<f32>) -> MapCoords {
    return coords_with(material_mapping(), uv, position, normal);
}

fn coords_with(m: Mapping, uv: vec2<f32>, position: vec3<f32>, normal: vec3<f32>) -> MapCoords {
    var c: MapCoords;
    let p = position * m.world_scale;
    c.uv = select(tile_with(m, p.xz), tile_with(m, uv), m.uv_mapping == UV0);
    c.x = c.uv;
    c.y = c.uv;
    c.z = c.uv;
    c.weights = vec3<f32>(0.0, 1.0, 0.0);
    c.shift = vec2<f32>(0.0);
    c.dx = dpdx(c.uv);
    c.dy = dpdy(c.uv);
    c.bend = 0.0;
    if (m.uv_mapping == TRIPLANAR) {
        c.x = tile_with(m, p.zy);
        c.z = tile_with(m, p.xy);
        let w = pow(max(abs(normal), vec3<f32>(1e-4)), vec3<f32>(m.blend_sharpness));
        c.weights = w / max(w.x + w.y + w.z, 1e-4);
    }
    return c;
}

fn sample_any(t: texture_2d<f32>, s: sampler, m: Mapping, uv: vec2<f32>, position: vec3<f32>, normal: vec3<f32>) -> vec4<f32> {
    let c = coords_with(m, uv, position, normal);
    let dx = mat3x2<f32>(dpdx(c.x), dpdx(c.y), dpdx(c.z));
    let dy = mat3x2<f32>(dpdy(c.x), dpdy(c.y), dpdy(c.z));
    let y = textureSampleGrad(t, s, c.y, dx[1], dy[1]);
    if (m.uv_mapping != TRIPLANAR) {
        return y;
    }
    return textureSampleGrad(t, s, c.x, dx[0], dy[0]) * c.weights.x
        + y * c.weights.y
        + textureSampleGrad(t, s, c.z, dx[2], dy[2]) * c.weights.z;
}

fn lookup(t: texture_2d<f32>, s: sampler, uv: vec2<f32>) -> vec4<f32> {
    let bias = camera.lod.x;
    if (bias == 0.0) {
        return textureSample(t, s, uv);
    }
    return textureSampleBias(t, s, uv, bias);
}

fn lookup_uv(t: texture_2d<f32>, s: sampler, c: MapCoords) -> vec4<f32> {
    let implicit = lookup(t, s, c.uv);
    if (c.bend <= 0.0) {
        return implicit;
    }
    let k = exp2(camera.lod.x);
    return mix(implicit, textureSampleGrad(t, s, c.uv, c.dx * k, c.dy * k), c.bend);
}

fn sample(t: texture_2d<f32>, s: sampler, c: MapCoords) -> vec4<f32> {
    if (material().uv_mapping != TRIPLANAR) {
        return lookup_uv(t, s, c);
    }
    return lookup(t, s, c.x) * c.weights.x
        + lookup(t, s, c.y) * c.weights.y
        + lookup(t, s, c.z) * c.weights.z;
}

// The factor metallic or roughness map `t` at `c` multiplies its scalar by, under its per-draw
// `use_*_map` flag `on` (`maps::map_factor`, `packed` its glTF channel): 1 without a map, which
// is then never fetched.
fn mask(on: u32, t: texture_2d<f32>, s: sampler, c: MapCoords, packed: u32) -> f32 {
    if (on == 0u) {
        return 1.0;
    }
    return map_factor(sample(t, s, c), on, packed);
}

// The occlusion map's factor at `c` (glTF's `occlusionTexture`): its red, lerped from 1 by the
// material's strength (`1 + strength * (occlusion - 1)`); 1 without a map, which is then never
// fetched.
fn occlusion(c: MapCoords) -> f32 {
    let m = material();
    return 1.0 + m.occlusion_strength * (mask(m.use_occlusion_map, t_occlusion, s_occlusion, c, 0u) - 1.0);
}

// The dielectric's reflectance at `c`: its F0 in rgb and F90 in w, both scaled by the
// specular-level map (`KHR_materials_specular`'s `specularTexture`: alpha, red at 2), as the
// per-material `specular` scales them.
fn dielectric_reflectance(c: MapCoords) -> vec4<f32> {
    let m = material();
    return m.dielectric_reflectance * mask(m.use_specular_map, t_specular, s_specular, c, 3u);
}

fn level_at(size: vec2<f32>, dx: vec2<f32>, dy: vec2<f32>) -> f32 {
    let a = dx * size;
    let b = dy * size;
    return 0.5 * log2(max(abs(a.x * b.y - a.y * b.x), 1e-12));
}

fn normal_level(t: texture_2d<f32>, c: MapCoords) -> f32 {
    let size = vec2<f32>(textureDimensions(t));
    var level = level_at(size, c.dx, c.dy);
    if (material().uv_mapping == TRIPLANAR) {
        let x = level_at(size, dpdx(c.x), dpdy(c.x));
        let y = level_at(size, dpdx(c.y), dpdy(c.y));
        let z = level_at(size, dpdx(c.z), dpdy(c.z));
        level = dot(vec3<f32>(x, y, z), c.weights);
    }
    return level + camera.lod.x;
}

struct NormalSample {
    normal: vec3<f32>,
    length: f32,
};

// The length of the normal a texel stores, z from its blue: under 1 where a mip averaged
// normals that disagreed.
fn stored_length(texel: vec4<f32>) -> f32 {
    let n = texel.xyz * 2.0 - 1.0;
    return select(length(n), 1.0, n.z < WHOLE_BELOW_Z);
}

const WHOLE_BELOW_Z: f32 = 0.25;

fn sample_normal(t: texture_2d<f32>, s: sampler, c: MapCoords, n: vec3<f32>, tangent: vec4<f32>) -> NormalSample {
    let mapping = material().uv_mapping;
    let texel_y = lookup_uv(t, s, c);
    if (mapping == UV0) {
        let T = normalize(tangent.xyz);
        let B = cross(n, T) * tangent.w;
        let sampled = unpack_normal(texel_y);
        return NormalSample(normalize(mat3x3<f32>(T, B, n) * sampled), stored_length(texel_y));
    }
    let ty = unpack_normal(texel_y);
    var tx = vec3<f32>(0.0, 0.0, 1.0);
    var tz = vec3<f32>(0.0, 0.0, 1.0);
    var w = vec3<f32>(0.0, 1.0, 0.0);
    var lengths = vec3<f32>(1.0, stored_length(texel_y), 1.0);
    if (mapping == TRIPLANAR) {
        let texel_x = lookup(t, s, c.x);
        let texel_z = lookup(t, s, c.z);
        tx = unpack_normal(texel_x);
        tz = unpack_normal(texel_z);
        lengths = vec3<f32>(stored_length(texel_x), lengths.y, stored_length(texel_z));
        w = c.weights;
    }
    return NormalSample(whiteout(tx, ty, tz, w, n), dot(lengths, w));
}

fn whiteout(tx: vec3<f32>, ty: vec3<f32>, tz: vec3<f32>, w: vec3<f32>, n: vec3<f32>) -> vec3<f32> {
    let nx = vec3<f32>(tx.xy + n.zy, abs(tx.z) * n.x);
    let ny = vec3<f32>(ty.xy + n.xz, abs(ty.z) * n.y);
    let nz = vec3<f32>(tz.xy + n.xy, abs(tz.z) * n.z);
    return normalize(nx.zyx * w.x + ny.xzy * w.y + nz.xyz * w.z);
}
