// Unity Lit's Tiling/Offset on UV0, or HDRP Lit's Planar and Triplanar *Base UV Mapping*.
// TAA's mip bias (`camera.lod.x`) goes to material maps, decals and the Toksvig level; never
// the lightmap, AO or the specular-AA derivatives.

#define_import_path forward::mapping

#import forward::bindings::{camera, material, s_occlusion, s_specular, t_occlusion, t_specular}
#import forward::maps::{map_factor, stored_normal, unpack_normal}

const UV0: u32 = 0u;
const TRIPLANAR: u32 = 2u;
// `render::gpu::textures::deep::SIGNED_NORMAL_MAP`.
const SIGNED_NORMAL_MAP: u32 = 3u;

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

// `draw::uniforms::mapping_uniform` flips UV0's v in `uv_st`, so this plain sum matches Unity's.
fn tile_with(m: Mapping, uv: vec2<f32>) -> vec2<f32> {
    return uv * m.uv_st.xy + m.uv_st.zw;
}

fn coords(uv: vec2<f32>, position: vec3<f32>, normal: vec3<f32>) -> MapCoords {
    return coords_with(material_mapping(), uv, position, normal);
}

// Takes derivatives. `world_scale` is repeats per metre. Triplanar's projections (zy, xz, xy)
// are Golus's, which `whiteout` expects; UV0 and Planar weigh Y alone.
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

// A per-instance mapping (a shadow caster's) takes its gradients first, so its branch samples
// explicitly.
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

// Explicit gradients only where the bend is above 0: some GPUs drop anisotropy on them
// (lavapipe) and shimmer at grazing angles. The bend is per pixel, so only `textureSampleGrad`
// may sit in its branch.
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

// `packed` is the glTF channel; 1 without a map, which is then never fetched.
fn mask(on: u32, t: texture_2d<f32>, s: sampler, c: MapCoords, packed: u32) -> f32 {
    if (on == 0u) {
        return 1.0;
    }
    return map_factor(sample(t, s, c), on, packed);
}

// glTF's `occlusionTexture`.
fn occlusion(c: MapCoords) -> f32 {
    let m = material();
    return 1.0 + m.occlusion_strength * (mask(m.use_occlusion_map, t_occlusion, s_occlusion, c, 0u) - 1.0);
}

// F0 in rgb and F90 in w, both scaled by the specular-level map (alpha, or red at 2).
fn dielectric_reflectance(c: MapCoords) -> vec4<f32> {
    let m = material();
    return m.dielectric_reflectance * mask(m.use_specular_map, t_specular, s_specular, c, 3u);
}

// The footprint's area, not its minor axis: anisotropic filtering averages the whole
// footprint's normals.
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

// Under 1 where a mip averaged normals that disagreed.
fn stored_length(n: vec3<f32>) -> f32 {
    return select(length(n), 1.0, n.z < WHOLE_BELOW_Z);
}

// A z under this is no normal's z (an RG export, packed AO, a BC5 zero).
const WHOLE_BELOW_Z: f32 = 0.25;

fn sample_normal(t: texture_2d<f32>, s: sampler, c: MapCoords, n: vec3<f32>, tangent: vec4<f32>) -> NormalSample {
    let m = material();
    let mapping = m.uv_mapping;
    let signed = m.use_normal_map == SIGNED_NORMAL_MAP;
    let stored_y = stored_normal(lookup_uv(t, s, c), signed);
    if (mapping == UV0) {
        let T = normalize(tangent.xyz);
        let B = cross(n, T) * tangent.w;
        let sampled = unpack_normal(stored_y);
        return NormalSample(normalize(mat3x3<f32>(T, B, n) * sampled), stored_length(stored_y));
    }
    let ty = unpack_normal(stored_y);
    var tx = vec3<f32>(0.0, 0.0, 1.0);
    var tz = vec3<f32>(0.0, 0.0, 1.0);
    var w = vec3<f32>(0.0, 1.0, 0.0);
    var lengths = vec3<f32>(1.0, stored_length(stored_y), 1.0);
    if (mapping == TRIPLANAR) {
        let stored_x = stored_normal(lookup(t, s, c.x), signed);
        let stored_z = stored_normal(lookup(t, s, c.z), signed);
        tx = unpack_normal(stored_x);
        tz = unpack_normal(stored_z);
        lengths = vec3<f32>(stored_length(stored_x), lengths.y, stored_length(stored_z));
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
