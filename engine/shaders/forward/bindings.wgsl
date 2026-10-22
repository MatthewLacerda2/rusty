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
    // flags. metallic/roughness scale their scalar by the sampled channel; normal
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
};

// One instance of an instanced draw (#470), indexed by `instance_index`. Mirrors
// `InstanceData` (src/render/gpu/uniforms.rs) byte-for-byte. Only per-copy data lives
// here — the world matrix and the light-probe SH (#240: when `use_sh == 1` the ambient
// term is reconstructed from `sh`, xyz = RGB radiance). Material flags stay in the
// per-draw `EntityUniforms`, so texture sampling never depends on an instance value.
struct InstanceData {
    model_matrix: mat4x4<f32>,
    use_sh: u32,
    // Baked lightmap (#438): atlas page + 1 (0 for none), and the scale/offset from
    // the mesh's lightmap UV into that page.
    lightmap_page: u32,
    // 1 when the lightmap's direction page is bound (#810).
    lightmap_directional: u32,
    _ipad_b: u32, // not `_ipad2`: naga_oil rejects imported names ending `_<digit>`
    lightmap_st: vec4<f32>,
    sh: array<vec4<f32>, 9>,
};

@group(0) @binding(0)
var<uniform> camera: CameraUniforms;

@group(0) @binding(1)
var<uniform> lighting: LightingUniforms;

@group(0) @binding(2)
var t_skybox: texture_2d<f32>;
@group(0) @binding(3)
var s_skybox: sampler;

// Active reflection probe's prefiltered cubemap (#245). Sampled with `textureSampleLevel`
// (roughness -> mip) and box-projected parallax when `refl_has_cubemap > 0.5`; otherwise a
// 1x1 black fallback is bound and ignored (the skybox is sampled instead).
@group(0) @binding(4)
var t_refl_cube: texture_cube<f32>;
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
