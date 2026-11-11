// engine/shaders/forward/mapping.wgsl — where a surface reads its material maps
// (#1573): Unity Lit's Tiling/Offset on UV0, or HDRP Lit's Planar and Triplanar
// *Base UV Mapping*, projected from world space so a scaled primitive keeps one
// texel density on every face. Every map lookup of the forward shader (and its
// prepass and the selection mask) goes through `sample`, the normal map through
// `sample_normal`, so all maps of a material agree on where they are read.
//
// Every branch here is on `material()` (the draw's record, indexed by a per-draw
// uniform) or on the camera's mip bias, so the implicit-LOD samples sit in uniform
// control flow.
//
// The mip bias (`camera.lod.x`) is TAA's: `log2(scene/output)` while it upscales
// the view (#1702), plus the camera's TAA *Mip Bias* on every frame TAA runs
// (#1847); 0 without TAA. Every material map takes it through [`lookup`] (the
// maps read here and the surface blocks' `t_mask`), and the decals through their
// sampling footprint (`forward::lighting::fold_decals`); the lightmap, the AO
// target and the geometric specular AA's derivatives never do; the normal map's
// Toksvig level adds it (`normal_level`), as its lookup does. At 0 the lookup is the plain
// `textureSample`, so a view at no bias samples as it always did.

#define_import_path forward::mapping

#import forward::bindings::{camera, material}
#import forward::maps::{map_factor, unpack_normal}

const UV0: u32 = 0u;
const TRIPLANAR: u32 = 2u;

// Where one surface point reads its maps. `uv` is the one lookup of UV0 and
// Planar; `x`, `y`, `z` are Triplanar's three projections (along world X, Y and
// Z), blended by `weights`.
struct MapCoords {
    uv: vec2<f32>,
    x: vec2<f32>,
    y: vec2<f32>,
    z: vec2<f32>,
    weights: vec3<f32>,
};

// A material's mapping terms: `render::gpu::uniforms::MappingUniform` byte-for-byte.
// The forward draws read theirs from `material()`; a shadow caster carries its own per
// instance (#1594), so the `*_with` functions take them explicitly.
struct Mapping {
    uv_st: vec4<f32>,
    uv_mapping: u32,
    world_scale: f32,
    blend_sharpness: f32,
    _pad: u32,
};

// The draw's own terms, from its material record.
fn material_mapping() -> Mapping {
    let m = material();
    return Mapping(m.uv_st, m.uv_mapping, m.world_scale, m.blend_sharpness, 0u);
}

// Tiling and Offset applied. The CPU packs `uv_st` so this plain sum reads what
// Unity's `TRANSFORM_TEX` does (`render::draw::uniforms::mapping_uniform`, #1851).
fn tile_with(m: Mapping, uv: vec2<f32>) -> vec2<f32> {
    return uv * m.uv_st.xy + m.uv_st.zw;
}

// The coordinates at a point with mesh UV `uv`, world `position` and geometric
// world `normal`, under the draw's own terms (`coords_with`).
fn coords(uv: vec2<f32>, position: vec3<f32>, normal: vec3<f32>) -> MapCoords {
    return coords_with(material_mapping(), uv, position, normal);
}

// `coords` under terms `m`. UV0 does no projection maths; Planar projects once
// (along Y). Triplanar's projections follow Ben Golus's "Normal Mapping for a
// Triplanar Shader" (zy, xz, xy), which `sample_normal`'s whiteout blend expects.
// UV0 and Planar fill all three projections with their one lookup, its weight all
// on Y, so `sample_any` reads them without a branch on the mapping.
fn coords_with(m: Mapping, uv: vec2<f32>, position: vec3<f32>, normal: vec3<f32>) -> MapCoords {
    var c: MapCoords;
    let p = position * m.world_scale;
    c.uv = select(tile_with(m, p.xz), tile_with(m, uv), m.uv_mapping == UV0);
    c.x = c.uv;
    c.y = c.uv;
    c.z = c.uv;
    c.weights = vec3<f32>(0.0, 1.0, 0.0);
    if (m.uv_mapping == TRIPLANAR) {
        c.x = tile_with(m, p.zy);
        c.z = tile_with(m, p.xy);
        let w = pow(max(abs(normal), vec3<f32>(1e-4)), vec3<f32>(m.blend_sharpness));
        c.weights = w / max(w.x + w.y + w.z, 1e-4);
    }
    return c;
}

// Map `t` under terms `m` at a point (`coords_with`'s arguments) where the mapping
// is not uniform across the draw (a shadow caster's, per instance): the derivatives
// are taken first, in uniform control flow, and the branch on the mapping samples
// with them.
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

// One material-map lookup at `uv`, with the camera's mip bias when it has one.
// The branch is on a uniform, so both samples are in uniform control flow.
fn lookup(t: texture_2d<f32>, s: sampler, uv: vec2<f32>) -> vec4<f32> {
    let bias = camera.lod.x;
    if (bias == 0.0) {
        return textureSample(t, s, uv);
    }
    return textureSampleBias(t, s, uv, bias);
}

// Map `t` at `c`.
fn sample(t: texture_2d<f32>, s: sampler, c: MapCoords) -> vec4<f32> {
    if (material().uv_mapping != TRIPLANAR) {
        return lookup(t, s, c.uv);
    }
    return lookup(t, s, c.x) * c.weights.x
        + lookup(t, s, c.y) * c.weights.y
        + lookup(t, s, c.z) * c.weights.z;
}

// The factor metallic or roughness map `t` at `c` multiplies its scalar by, under
// its per-draw `use_*_map` flag `on` (`maps::map_factor`, `packed` its glTF
// channel): 1 without a map, which is then never fetched.
fn mask(on: u32, t: texture_2d<f32>, s: sampler, c: MapCoords, packed: u32) -> f32 {
    if (on == 0u) {
        return 1.0;
    }
    return map_factor(sample(t, s, c), on, packed);
}

// The mip level whose texel covers as many texels of a `size`-texel texture as one
// pixel does, from the screen derivatives `dx` and `dy` of its coordinates: half the
// log2 of the footprint's area. A grazing footprint is long and thin; an anisotropic
// sampler reads a sharp level along it and averages its taps, so the spread of
// normals a pixel covers is the whole footprint's, which this treats as a square of
// the same area (the sampler's own level, the minor axis's, would see almost none).
fn level_at(size: vec2<f32>, dx: vec2<f32>, dy: vec2<f32>) -> f32 {
    let a = dx * size;
    let b = dy * size;
    return 0.5 * log2(max(abs(a.x * b.y - a.y * b.x), 1e-12));
}

// The level of normal map `t` one pixel's footprint covers at `c` (`level_at`):
// UV0's and Planar's one lookup's, Triplanar's three projections' blended by their
// weights, plus the mip bias [`lookup`] reads it with. Takes derivatives: call it in
// uniform control flow (a branch on `material()` is).
fn normal_level(t: texture_2d<f32>, c: MapCoords) -> f32 {
    let size = vec2<f32>(textureDimensions(t));
    var level = level_at(size, dpdx(c.uv), dpdy(c.uv));
    if (material().uv_mapping == TRIPLANAR) {
        let x = level_at(size, dpdx(c.x), dpdy(c.x));
        let y = level_at(size, dpdx(c.y), dpdy(c.y));
        let z = level_at(size, dpdx(c.z), dpdy(c.z));
        level = dot(vec3<f32>(x, y, z), c.weights);
    }
    return level + camera.lod.x;
}

// A normal map's filtered read: the world-space shading normal, and how long the
// stored normal the texels averaged to is (`stored_length`), blended as the samples
// are. The length means something only for a map that keeps z (`texture_kernel`).
struct NormalSample {
    normal: vec3<f32>,
    length: f32,
};

// The length of the normal a texel stores, z from its blue: under 1 where a mip
// averaged normals that disagreed. A texel whose stored z is under `WHOLE_BELOW_Z`
// counts as whole (length 1): a mip of upward normals never averages that low, so
// such a blue is not the normal's z (an RG export's 128, AO or height packed in
// blue, a BC5 texel's 0), and its length would be a false spread (#1702's review).
fn stored_length(texel: vec4<f32>) -> f32 {
    let n = texel.xyz * 2.0 - 1.0;
    return select(length(n), 1.0, n.z < WHOLE_BELOW_Z);
}

const WHOLE_BELOW_Z: f32 = 0.25;

// The world-space shading normal from tangent-space normal map `t` at `c`, over
// geometric normal `n`, each sample's z rebuilt from x and y (`maps::unpack_normal`,
// so a BC5 map, which keeps only those, reads right, #1574). UV0 rotates the map by the mesh's tangent frame (`tangent`,
// `w` its handedness); Planar and Triplanar have no mesh tangents to lean on, so
// each projection's sample is reoriented onto its world axis with the whiteout
// blend (Golus), and Planar is Triplanar with all the weight on the Y projection.
fn sample_normal(t: texture_2d<f32>, s: sampler, c: MapCoords, n: vec3<f32>, tangent: vec4<f32>) -> NormalSample {
    let mapping = material().uv_mapping;
    let texel_y = lookup(t, s, c.uv);
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

// Tangent-space normals `tx`, `ty`, `tz`, read along world X, Y and Z, reoriented
// onto normal `n` and blended by `w` (Golus's whiteout blend): a world mapping's
// normal. `sample_normal` reads the main normal map through it, and the detail
// layer (`forward::detail`) its own over the main shading normal.
fn whiteout(tx: vec3<f32>, ty: vec3<f32>, tz: vec3<f32>, w: vec3<f32>, n: vec3<f32>) -> vec3<f32> {
    let nx = vec3<f32>(tx.xy + n.zy, abs(tx.z) * n.x);
    let ny = vec3<f32>(ty.xy + n.xz, abs(ty.z) * n.y);
    let nz = vec3<f32>(tz.xy + n.xy, abs(tz.z) * n.z);
    return normalize(nx.zyx * w.x + ny.xzy * w.y + nz.xyz * w.z);
}
