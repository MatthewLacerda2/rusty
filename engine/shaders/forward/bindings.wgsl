// engine/shaders/forward/bindings.wgsl — every resource the forward pass binds, and
// the per-draw and per-instance data they hold (#1015). `shader.wgsl` and its baked
// surface variants import them; so does `forward/lighting.wgsl`, whose helpers read
// them. One declaration, so a binding is never declared twice.

#define_import_path forward::bindings

#import common::{CameraUniforms, Decal, LightingUniforms, LocalLight, ShadowTile, VertexInput, blend_joints}

struct EntityUniforms {
    model_matrix: mat4x4<f32>,
    color_tint: vec4<f32>,
    use_texture: u32,
    is_lit: u32,
    metallic: f32,
    roughness: f32,
    // Mirrors `EntityUniform` (src/render/mod.rs) byte-for-byte (#202, #207): map
    // flags. metallic/roughness scale their scalar by the sampled channel (glTF's
    // blue/green at 1, red at 2: a one-channel BC4 upload, #1574); normal
    // perturbs the shading normal; emissive modulates the emissive factor. Four u32s
    // fill the run so the `vec4` that follows is 16-byte aligned.
    use_metallic_map: u32,
    use_roughness_map: u32,
    use_normal_map: u32,
    use_emissive_map: u32,
    // Flat emissive factor (#222): rgb glow added after lighting in `fs_main`; the
    // 4th lane is unused. `vec4` to match the Rust `[f32; 4]` byte-for-byte.
    emissive: vec4<f32>,
    // Cutout alpha-test (#242): `use_cutout == 1` discards fragments whose final
    // alpha is below `alpha_cutoff`.
    use_cutout: u32,
    alpha_cutoff: f32,
    // Where this draw's joint 0 sits in `bones` (#455); 0 is the shared identity.
    // Mirrors `EntityUniform` (src/render/gpu/uniforms.rs) byte-for-byte.
    bone_base: u32,
    // 1 when this draw folds in its cluster's decals (#638).
    receive_decals: u32,
    // The transparent-surface terms (#1523), mirroring `SurfaceUniform`
    // (src/render/gpu/uniforms.rs) byte-for-byte. `blend_mode`: 0 Alpha,
    // 1 Premultiply, 2 Additive, 3 Multiply.
    blend_mode: u32,
    // 1 when preserve specular applies: diffuse * alpha here, specular whole.
    premultiply_diffuse: u32,
    receive_shadows: u32,
    // 0 none, 1 planar (HDRP's Box), 2 thin.
    refraction_model: u32,
    ior: f32,
    thickness: f32,
    // 1 when the surface composites over the opaque copy itself.
    uses_opaque: u32,
    depth_postpass: u32,
    // Absorption per metre (rgb); w the depth it absorbs over against the sky.
    absorption: vec4<f32>,
    // Thin translucency: rgb colour * amount, w the amount.
    transmission: vec4<f32>,
    // 1 when the surface traces screen-space reflections (#1570) where the camera
    // marches (`camera.stack.z`).
    receive_ssr: u32,
    surface_pad_a: u32,
    surface_pad_b: u32,
    surface_pad_c: u32,
    // Where the maps are read (#1573), mirroring `MappingUniform`
    // (src/render/gpu/uniforms.rs) byte-for-byte: xy the tiling, zw the offset.
    uv_st: vec4<f32>,
    // 0 UV0, 1 planar (world XZ), 2 triplanar (`forward/mapping.wgsl`).
    uv_mapping: u32,
    // Texture repeats per metre under a world mapping.
    world_scale: f32,
    blend_sharpness: f32,
    mapping_pad: u32,
};

// One instance of an instanced draw (#470), indexed by `instance_index`. Mirrors
// `InstanceData` (src/render/gpu/uniforms.rs) byte-for-byte. Only per-copy data lives
// here — the world matrix and the light-probe SH (#240: when `use_sh == 1` the ambient
// term is reconstructed from `sh`, xyz = RGB radiance). Material flags stay in the
// per-draw `EntityUniforms`, so texture sampling never depends on an instance value.
struct InstanceData {
    model_matrix: mat4x4<f32>,
    // Last frame's world matrix, read only when `motion == 1` (#1435).
    prev_model_matrix: mat4x4<f32>,
    use_sh: u32,
    // Baked lightmap (#438): atlas page + 1 (0 for none), and the scale/offset from
    // the mesh's lightmap UV into that page.
    lightmap_page: u32,
    // 1 when the lightmap's direction page is bound (#810).
    lightmap_directional: u32,
    // The resident reflection probe this instance reflects, plus one; 0 for none
    // (the sky). Picked per object at its world-bounds centre (#1206).
    reflection_probe: u32,
    lightmap_st: vec4<f32>,
    // Motion vectors (#1435): where last frame's joint palette starts in `bones`, and
    // 1 when `prev_model_matrix` and it hold last frame's pose. 0 (no history: a first
    // draw, a camera cut, a mesh particle) moves the vertex with the camera only.
    prev_bone_base: u32,
    motion: u32,
    motion_pad_a: u32,
    motion_pad_b: u32,
    sh: array<vec4<f32>, 9>,
};

@group(0) @binding(0)
var<uniform> camera: CameraUniforms;

@group(0) @binding(1)
var<uniform> lighting: LightingUniforms;

// The sky's reflection cube (#1553, `render::ibl::sky_cube`): the panorama, or the
// procedural sky's two terms (rgb at tint 0, a the tint's weight), GGX-prefiltered
// into a roughness mip chain like a probe's cube.
@group(0) @binding(2)
var t_skybox: texture_cube<f32>;
@group(0) @binding(3)
var s_skybox: sampler;

// The resident reflection probes' prefiltered cubemaps (#245), one per layer (#1206),
// sampled with `textureSampleLevel` (roughness -> mip) at the layer a draw's
// `reflection_probe` names. A draw with none samples the sky's cube instead.
@group(0) @binding(4)
var t_refl_cube: texture_cube_array<f32>;
@group(0) @binding(5)
var s_refl_cube: sampler;

// Clustered lights (#434): the frame's point/spot lights, each cluster's
// (offset, count) into `cluster_lights`, and that flat list of light indices.
// Binding 6 is the shadow module's (below).
@group(0) @binding(7)
var<storage, read> local_lights: array<LocalLight>;
@group(0) @binding(8)
var<storage, read> cluster_ranges: array<vec2<u32>>;
@group(0) @binding(9)
var<storage, read> cluster_lights: array<u32>;

// Surface decals (#638): the frame's decals, and the atlas their maps live in, read
// through its sRGB view (albedo) and its raw view (normal, metallic, roughness). A
// cluster's decals follow the light ranges in `cluster_ranges`, indexing
// `cluster_lights` like its lights do.
@group(0) @binding(10)
var<storage, read> decals: array<Decal>;
@group(0) @binding(11)
var t_decal_color: texture_2d_array<f32>;
@group(0) @binding(12)
var t_decal_data: texture_2d_array<f32>;
@group(0) @binding(13)
var s_decal: sampler;

// Baked lightmap atlas pages (#438): linear RGBM, one layer per page, read at the
// instance's page through its scale/offset; a 1x1 black page when none are baked.
@group(0) @binding(14)
var t_lightmaps: texture_2d_array<f32>;
@group(0) @binding(15)
var s_lightmaps: sampler;
// Their direction pages (#810), same layers and UVs: RGB the dominant incoming
// direction in [0,1], A its directionality; a 1x1 zero page when not baked.
@group(0) @binding(16)
var t_lightmap_dirs: texture_2d_array<f32>;

@group(1) @binding(0)
var<uniform> entity: EntityUniforms;

// Every skinned draw's joint matrices back to back (#455), sized per skin — no joint
// cap. A draw reads its run from `entity.bone_base`; element 0 is the identity.
@group(1) @binding(1)
var<storage, read> bones: array<mat4x4<f32>>;

@group(1) @binding(2)
var<storage, read> instances: array<InstanceData>;

// Where `model`'s vertex goes in world space: its skin (the draw's run of `bones`,
// #455) under the draw's transform (identity for instanced solids) over the
// instance's own. The forward pass and the selection outline (#1175) both place a
// vertex through this, so the outline cannot sit anywhere the solid does not.
fn vertex_to_world(model: VertexInput, instance: u32) -> mat4x4<f32> {
    let joints = model.joint_indices + vec4<u32>(entity.bone_base);
    let bone_transform = blend_joints(
        bones[joints.x], bones[joints.y], bones[joints.z], bones[joints.w],
        model.joint_weights,
    );
    return entity.model_matrix * instances[instance].model_matrix * bone_transform;
}

// `normal` (object space) to world space under `to_world` (#1596): by the
// inverse-transpose of its upper 3x3, as Unity's `unity_WorldToObject` does, so a
// non-uniformly scaled curved mesh keeps its normals perpendicular to the surface.
// The cofactor matrix (each column the cross product of the other two) is that
// inverse-transpose times the determinant, so after `normalize` it points the same
// way for three cross products and no per-draw uniform; `sign(det)` keeps a mirrored
// (negative-determinant) placement's normals outward.
fn normal_to_world(to_world: mat4x4<f32>, normal: vec3<f32>) -> vec3<f32> {
    let c0 = to_world[0].xyz;
    let c1 = to_world[1].xyz;
    let c2 = to_world[2].xyz;
    let cofactor = mat3x3<f32>(cross(c1, c2), cross(c2, c0), cross(c0, c1));
    let det = dot(c0, cross(c1, c2));
    return normalize(cofactor * normal) * select(1.0, -1.0, det < 0.0);
}

// `tangent` (object space) to world space under `to_world`: it lies in the surface,
// so it rides `to_world` itself, then is re-orthogonalised against the world
// `normal`. One along the normal has no in-surface part and is kept as transformed.
// Its handedness (`w`) is the caller's to pass on.
fn tangent_to_world(to_world: mat4x4<f32>, tangent: vec3<f32>, normal: vec3<f32>) -> vec3<f32> {
    let along = (to_world * vec4<f32>(tangent, 0.0)).xyz;
    let ortho = along - normal * dot(normal, along);
    return normalize(select(ortho, along, dot(ortho, ortho) < 1e-12));
}

// Where `model`'s vertex was in world space last frame (#1435): last frame's palette
// under last frame's world matrix, or this frame's placement when the instance has no
// history, so it moves with the camera alone.
fn vertex_to_world_prev(model: VertexInput, instance: u32) -> mat4x4<f32> {
    if (instances[instance].motion == 0u) {
        return vertex_to_world(model, instance);
    }
    let joints = model.joint_indices + vec4<u32>(instances[instance].prev_bone_base);
    let bone_transform = blend_joints(
        bones[joints.x], bones[joints.y], bones[joints.z], bones[joints.w],
        model.joint_weights,
    );
    return entity.model_matrix * instances[instance].prev_model_matrix * bone_transform;
}

// The material maps, each with its own sampler (#1323): wrap and filter are the
// texture's import settings. A map's sampler sits 16 bindings past it
// (`shadergen::textures::sampler_binding`); albedo's stays at 1.
@group(2) @binding(0)
var t_diffuse: texture_2d<f32>;
@group(2) @binding(1)
var s_diffuse: sampler;
@group(2) @binding(2)
var t_metallic: texture_2d<f32>;
@group(2) @binding(3)
var t_roughness: texture_2d<f32>;
@group(2) @binding(4)
var t_normal: texture_2d<f32>;
@group(2) @binding(5)
var t_emissive: texture_2d<f32>;
@group(2) @binding(18)
var s_metallic: sampler;
@group(2) @binding(19)
var s_roughness: sampler;
@group(2) @binding(20)
var s_normal: sampler;
@group(2) @binding(21)
var s_emissive: sampler;

// The sun's cascaded shadow maps (#435): one light volume per slice of the view,
// written by `render::passes::shadows::CascadeUniform`.
struct ShadowCascades {
    light_space: array<mat4x4<f32>, 4>,
    splits: vec4<f32>,       // view depth where each cascade ends
    texels: vec4<f32>,       // world size of one texel, per cascade
    depth_ranges: vec4<f32>, // world depth each cascade's [0, 1] spans
    view_pos: vec4<f32>,
    view_dir: vec4<f32>,
    params: vec4<f32>,       // x count, y shadow distance, z blend fraction
};

@group(3) @binding(0)
var<uniform> shadow: ShadowCascades;
@group(3) @binding(1)
var t_shadow: texture_depth_2d_array;
@group(3) @binding(2)
var s_shadow: sampler_comparison;
// Screen-space ambient occlusion (#436), one texel per pixel; a 1x1 white texture
// wherever the SSAO pass did not run (off, Low tier, the transparent pass).
@group(3) @binding(3)
var t_ao: texture_2d<f32>;
// The point/spot shadow atlas (#468) and its tiles; a light's `shadow` indexes them.
@group(3) @binding(4)
var t_shadow_atlas: texture_depth_2d;
@group(3) @binding(5)
var<storage, read> shadow_tiles: array<ShadowTile>;
// The camera's opaque copy (#1523, `opaque_copy.wgsl`), Unity's *Opaque Texture*
// and *Depth Texture* in one: rgb its colour after the opaque pass and SSR, alpha
// the distance from the eye to that surface; a 1x1 stand-in in every pass but a
// transparent one with a surface that refracts or absorbs. Linear, clamped sampler.
@group(3) @binding(6)
var t_opaque: texture_2d<f32>;
@group(3) @binding(7)
var s_opaque: sampler;
