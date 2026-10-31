// engine/shaders/forward/mapping.wgsl — where a surface reads its material maps
// (#1573): Unity Lit's Tiling/Offset on UV0, or HDRP Lit's Planar and Triplanar
// *Base UV Mapping*, projected from world space so a scaled primitive keeps one
// texel density on every face. Every map lookup of the forward shader (and its
// prepass and the selection mask) goes through `sample`, the normal map through
// `sample_normal`, so all maps of a material agree on where they are read.
//
// Every branch here is on `material()` (the draw's record, indexed by a per-draw
// uniform), so the implicit-LOD samples sit in uniform control flow.

#define_import_path forward::mapping

#import forward::bindings::material
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

// The draw's own terms.
fn entity_mapping() -> Mapping {
    let m = material();
    return Mapping(m.uv_st, m.uv_mapping, m.world_scale, m.blend_sharpness, 0u);
}

fn tile_with(m: Mapping, uv: vec2<f32>) -> vec2<f32> {
    return uv * m.uv_st.xy + m.uv_st.zw;
}

// The coordinates at a point with mesh UV `uv`, world `position` and geometric
// world `normal`, under the draw's own terms (`coords_with`).
fn coords(uv: vec2<f32>, position: vec3<f32>, normal: vec3<f32>) -> MapCoords {
    return coords_with(entity_mapping(), uv, position, normal);
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

// Map `t` at `c`.
fn sample(t: texture_2d<f32>, s: sampler, c: MapCoords) -> vec4<f32> {
    if (material().uv_mapping != TRIPLANAR) {
        return textureSample(t, s, c.uv);
    }
    return textureSample(t, s, c.x) * c.weights.x
        + textureSample(t, s, c.y) * c.weights.y
        + textureSample(t, s, c.z) * c.weights.z;
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

// The world-space shading normal from tangent-space normal map `t` at `c`, over
// geometric normal `n`, each sample's z rebuilt from x and y (`maps::unpack_normal`,
// so a BC5 map, which keeps only those, reads right, #1574). UV0 rotates the map by the mesh's tangent frame (`tangent`,
// `w` its handedness); Planar and Triplanar have no mesh tangents to lean on, so
// each projection's sample is reoriented onto its world axis with the whiteout
// blend (Golus), and Planar is Triplanar with all the weight on the Y projection.
fn sample_normal(t: texture_2d<f32>, s: sampler, c: MapCoords, n: vec3<f32>, tangent: vec4<f32>) -> vec3<f32> {
    if (material().uv_mapping == UV0) {
        let T = normalize(tangent.xyz);
        let B = cross(n, T) * tangent.w;
        let sampled = unpack_normal(textureSample(t, s, c.uv));
        return normalize(mat3x3<f32>(T, B, n) * sampled);
    }
    let ty = unpack_normal(textureSample(t, s, c.uv));
    var tx = vec3<f32>(0.0, 0.0, 1.0);
    var tz = vec3<f32>(0.0, 0.0, 1.0);
    var w = vec3<f32>(0.0, 1.0, 0.0);
    if (material().uv_mapping == TRIPLANAR) {
        tx = unpack_normal(textureSample(t, s, c.x));
        tz = unpack_normal(textureSample(t, s, c.z));
        w = c.weights;
    }
    let nx = vec3<f32>(tx.xy + n.zy, abs(tx.z) * n.x);
    let ny = vec3<f32>(ty.xy + n.xz, abs(ty.z) * n.y);
    let nz = vec3<f32>(tz.xy + n.xy, abs(tz.z) * n.z);
    return normalize(nx.zyx * w.x + ny.xzy * w.y + nz.xyz * w.z);
}
