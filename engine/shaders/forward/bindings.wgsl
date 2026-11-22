
#define_import_path forward::bindings

#import common::{CameraUniforms, Decal, LightingUniforms, LocalLight, ShadowTile, VertexInput, blend_joints}
#import lighting::cascades::ShadowCascades

// Mirrors `DrawUniform` byte for byte: the draw's transform over each instance's (identity
// for instanced solids), its joint 0 in `bones` (0 the shared identity), its `materials` record.
struct DrawUniforms {
    model_matrix: mat4x4<f32>,
    bone_base: u32,
    material: u32,
    draw_pad_a: u32,
    draw_pad_b: u32,
};

// Mirrors `MaterialData` byte for byte, its `SurfaceUniform` and `MappingUniform` flat.
struct MaterialData {
    color_tint: vec4<f32>,
    use_texture: u32,
    is_lit: u32,
    metallic: f32,
    roughness: f32,
    use_metallic_map: u32,
    use_roughness_map: u32,
    // 0 none, 1 keeps z, 2 BC5 (`normal_variance`), 3 signed.
    use_normal_map: u32,
    use_emissive_map: u32,
    emissive: vec4<f32>,
    use_cutout: u32,
    alpha_cutoff: f32,
    receive_decals: u32,
    two_sided: u32,
    // 0 Alpha, 1 Premultiply, 2 Additive, 3 Multiply.
    blend_mode: u32,
    premultiply_diffuse: u32,
    receive_shadows: u32,
    // 0 none, 1 planar (HDRP's Box), 2 thin.
    refraction_model: u32,
    ior: f32,
    thickness: f32,
    uses_opaque: u32,
    depth_postpass: u32,
    absorption: vec4<f32>,
    transmission: vec4<f32>,
    receive_ssr: u32,
    reactive: f32,
    // HDRP's *Transparent Writes Motion Vectors*; read on the CPU.
    writes_motion: u32,
    surface_pad_c: u32,
    uv_st: vec4<f32>,
    // 0 UV0, 1 planar (world XZ), 2 triplanar.
    uv_mapping: u32,
    world_scale: f32,
    blend_sharpness: f32,
    mapping_pad: u32,
    normal_variance: vec4<u32>,
    dielectric_reflectance: vec4<f32>,
    detail_st: vec4<f32>,
    use_detail_map: u32,
    detail_world_scale: f32,
    detail_albedo_scale: f32,
    detail_normal_scale: f32,
    detail_smoothness_scale: f32,
    use_detail_mask: u32,
    detail_pad_a: u32,
    detail_pad_b: u32,
    use_occlusion_map: u32,
    occlusion_strength: f32,
    use_specular_map: u32,
    use_height_map: u32,
    height_scale: f32,
    maps_pad_a: u32,
    maps_pad_b: u32,
    maps_pad_c: u32,
};

// Mirrors `ObjectData` byte for byte. Per instance, not uniform per draw: never gate a texture
// sample on its fields.
struct ObjectData {
    model_matrix: mat4x4<f32>,
    use_sh: u32,
    // Lightmap atlas page + 1, 0 for none.
    lightmap_page: u32,
    lightmap_directional: u32,
    in_bake: u32,
    lightmap_st: vec4<f32>,
    sh: array<vec4<f32>, 9>,
    occlusion: vec4<f32>,
};

// Mirrors `InstanceData` byte for byte.
struct InstanceData {
    object: u32,
    reflection_probe: u32,
    proxy_volume: u32,
    motion: u32,
};

@group(0) @binding(0)
var<uniform> camera: CameraUniforms;

@group(0) @binding(1)
var<uniform> lighting: LightingUniforms;

@group(0) @binding(2)
var t_skybox: texture_cube<f32>;

@group(0) @binding(4)
var t_refl_cube: texture_cube_array<f32>;

// Binding 6 is `forward::shadow_caster`'s, as is group 3's binding 3.
@group(0) @binding(7)
var<storage, read> local_lights: array<LocalLight>;
@group(0) @binding(8)
var<storage, read> cluster_ranges: array<vec2<u32>>;
@group(0) @binding(9)
var<storage, read> cluster_lights: array<u32>;

@group(0) @binding(10)
var<storage, read> decals: array<Decal>;
@group(0) @binding(11)
var t_decal_color: texture_2d_array<f32>;
@group(0) @binding(12)
var t_decal_data: texture_2d_array<f32>;
@group(0) @binding(13)
var s_decal: sampler;

@group(0) @binding(14)
var t_lightmaps: texture_2d_array<f32>;
// Their direction pages (same layers, UVs): RGB the dominant incoming direction scaled by its
// directionality, A its lamp share (`decode_lightmap_direction`); a 1x1 zero page, never
// read, when not baked.
@group(0) @binding(16)
var t_lightmap_dirs: texture_2d_array<f32>;
@group(0) @binding(17)
var t_fog_volume: texture_3d<f32>;
@group(0) @binding(18)
var s_linear_clamp: sampler;
@group(0) @binding(19)
var t_lightmap_masks: texture_2d_array<f32>;

@group(3) @binding(0)
var<uniform> draw: DrawUniforms;

@group(3) @binding(1)
var<storage, read> bones: array<mat4x4<f32>>;

@group(3) @binding(2)
var<storage, read> instances: array<InstanceData>;

@group(3) @binding(6)
var<storage, read> objects: array<ObjectData>;

fn object_of(instance: u32) -> u32 {
    return instances[instance].object;
}

@group(3) @binding(5)
var<storage, read> proxy_volumes: array<vec4<f32>>;

@group(3) @binding(4)
var<storage, read> materials: array<MaterialData>;

// Indexed by the draw's uniform, so its flags are uniform per draw.
fn material() -> MaterialData {
    return materials[draw.material];
}

fn vertex_to_world(model: VertexInput, instance: u32) -> mat4x4<f32> {
    let joints = model.joint_indices + vec4<u32>(draw.bone_base);
    let bone_transform = blend_joints(
        bones[joints.x], bones[joints.y], bones[joints.z], bones[joints.w],
        model.joint_weights,
    );
    return draw.model_matrix * objects[object_of(instance)].model_matrix * bone_transform;
}

// The cofactor is the inverse-transpose times det, with no inverse; `sign(det)` keeps a
// mirrored placement's normals outward.
fn normal_to_world(to_world: mat4x4<f32>, normal: vec3<f32>) -> vec3<f32> {
    let c0 = to_world[0].xyz;
    let c1 = to_world[1].xyz;
    let c2 = to_world[2].xyz;
    let cofactor = mat3x3<f32>(cross(c1, c2), cross(c2, c0), cross(c0, c1));
    let det = dot(c0, cross(c1, c2));
    return normalize(cofactor * normal) * select(1.0, -1.0, det < 0.0);
}

fn tangent_to_world(to_world: mat4x4<f32>, tangent: vec3<f32>, normal: vec3<f32>) -> vec3<f32> {
    let along = (to_world * vec4<f32>(tangent, 0.0)).xyz;
    let ortho = along - normal * dot(normal, along);
    return normalize(select(ortho, along, dot(ortho, ortho) < 1e-12));
}

fn vertex_to_world_prev(model: VertexInput, instance: u32) -> mat4x4<f32> {
    let motion = instances[instance].motion;
    if (motion == 0u) {
        return vertex_to_world(model, instance);
    }
    let prev_base = select(0u, motion + 1u, draw.bone_base != 0u);
    let joints = model.joint_indices + vec4<u32>(prev_base);
    let bone_transform = blend_joints(
        bones[joints.x], bones[joints.y], bones[joints.z], bones[joints.w],
        model.joint_weights,
    );
    return draw.model_matrix * bones[motion] * bone_transform;
}

@group(1) @binding(0)
var t_diffuse: texture_2d<f32>;
@group(1) @binding(1)
var s_diffuse: sampler;
@group(1) @binding(2)
var t_metallic: texture_2d<f32>;
@group(1) @binding(3)
var t_roughness: texture_2d<f32>;
@group(1) @binding(4)
var t_normal: texture_2d<f32>;
@group(1) @binding(5)
var t_emissive: texture_2d<f32>;
@group(1) @binding(18)
var s_metallic: sampler;
@group(1) @binding(19)
var s_roughness: sampler;
@group(1) @binding(20)
var s_normal: sampler;
@group(1) @binding(21)
var s_emissive: sampler;
@group(1) @binding(15)
var t_detail: texture_2d<f32>;
@group(1) @binding(31)
var s_detail: sampler;
@group(1) @binding(14)
var t_occlusion: texture_2d<f32>;
@group(1) @binding(30)
var s_occlusion: sampler;
@group(1) @binding(13)
var t_specular: texture_2d<f32>;
@group(1) @binding(29)
var s_specular: sampler;
@group(1) @binding(12)
var t_height: texture_2d<f32>;
@group(1) @binding(28)
var s_height: sampler;
@group(1) @binding(11)
var t_detail_mask: texture_2d<f32>;
@group(1) @binding(27)
var s_detail_mask: sampler;

@group(2) @binding(0)
var<uniform> shadow: ShadowCascades;
@group(2) @binding(1)
var t_shadow: texture_depth_2d_array;
@group(2) @binding(2)
var s_shadow: sampler_comparison;
@group(2) @binding(3)
var t_ao: texture_2d<f32>;
@group(2) @binding(4)
var t_shadow_atlas: texture_depth_2d;
@group(2) @binding(5)
var<storage, read> shadow_tiles: array<ShadowTile>;
// The opaque copy, alpha the eye distance; in a base camera's opaque pass running SSIL
// (`camera.ssil.x`), SSIL's bounce instead.
@group(2) @binding(6)
var t_opaque: texture_2d<f32>;
@group(2) @binding(7)
var s_opaque: sampler;
