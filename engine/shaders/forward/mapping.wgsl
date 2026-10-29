// engine/shaders/forward/mapping.wgsl — where a surface reads its material maps
// (#1573): Unity Lit's Tiling/Offset on UV0, or HDRP Lit's Planar and Triplanar
// *Base UV Mapping*, projected from world space so a scaled primitive keeps one
// texel density on every face. Every map lookup of the forward shader (and its
// prepass and the selection mask) goes through `sample`, the normal map through
// `sample_normal`, so all maps of a material agree on where they are read.
//
// Every branch here is on `entity` (a per-draw uniform), so the implicit-LOD
// samples sit in uniform control flow.

#define_import_path forward::mapping

#import forward::bindings::entity

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

fn tile(uv: vec2<f32>) -> vec2<f32> {
    return uv * entity.uv_st.xy + entity.uv_st.zw;
}

// The coordinates at a point with mesh UV `uv`, world `position` and geometric
// world `normal`. The projections follow Ben Golus's "Normal Mapping for a
// Triplanar Shader" (zy, xz, xy), which `sample_normal`'s whiteout blend expects.
fn coords(uv: vec2<f32>, position: vec3<f32>, normal: vec3<f32>) -> MapCoords {
    let p = position * entity.world_scale;
    var c: MapCoords;
    c.x = tile(p.zy);
    c.y = tile(p.xz);
    c.z = tile(p.xy);
    c.uv = select(c.y, tile(uv), entity.uv_mapping == UV0);
    let w = pow(max(abs(normal), vec3<f32>(1e-4)), vec3<f32>(entity.blend_sharpness));
    c.weights = w / (w.x + w.y + w.z);
    return c;
}

// Map `t` at `c`.
fn sample(t: texture_2d<f32>, s: sampler, c: MapCoords) -> vec4<f32> {
    if (entity.uv_mapping != TRIPLANAR) {
        return textureSample(t, s, c.uv);
    }
    return textureSample(t, s, c.x) * c.weights.x
        + textureSample(t, s, c.y) * c.weights.y
        + textureSample(t, s, c.z) * c.weights.z;
}

// The world-space shading normal from tangent-space normal map `t` at `c`, over
// geometric normal `n`. UV0 rotates the map by the mesh's tangent frame (`tangent`,
// `w` its handedness); Planar and Triplanar have no mesh tangents to lean on, so
// each projection's sample is reoriented onto its world axis with the whiteout
// blend (Golus), and Planar is Triplanar with all the weight on the Y projection.
fn sample_normal(t: texture_2d<f32>, s: sampler, c: MapCoords, n: vec3<f32>, tangent: vec4<f32>) -> vec3<f32> {
    if (entity.uv_mapping == UV0) {
        let T = normalize(tangent.xyz);
        let B = cross(n, T) * tangent.w;
        let sampled = textureSample(t, s, c.uv).xyz * 2.0 - 1.0;
        return normalize(mat3x3<f32>(T, B, n) * sampled);
    }
    let ty = textureSample(t, s, c.uv).xyz * 2.0 - 1.0;
    var tx = vec3<f32>(0.0, 0.0, 1.0);
    var tz = vec3<f32>(0.0, 0.0, 1.0);
    var w = vec3<f32>(0.0, 1.0, 0.0);
    if (entity.uv_mapping == TRIPLANAR) {
        tx = textureSample(t, s, c.x).xyz * 2.0 - 1.0;
        tz = textureSample(t, s, c.z).xyz * 2.0 - 1.0;
        w = c.weights;
    }
    let nx = vec3<f32>(tx.xy + n.zy, abs(tx.z) * n.x);
    let ny = vec3<f32>(ty.xy + n.xz, abs(ty.z) * n.y);
    let nz = vec3<f32>(tz.xy + n.xy, abs(tz.z) * n.z);
    return normalize(nx.zyx * w.x + ny.xzy * w.y + nz.xyz * w.z);
}
