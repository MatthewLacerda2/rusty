// engine/shaders/forward/bindings.wgsl — every resource the forward pass binds, and
// the per-draw and per-instance data they hold (#1015). `shader.wgsl` and its baked
// surface variants import them; so does `forward/lighting.wgsl`, whose helpers read
// them. One declaration, so a binding is never declared twice.

#define_import_path forward::bindings

#import common::{CameraUniforms, Decal, LightingUniforms, LocalLight, ShadowTile, VertexInput, blend_joints}
#import lighting::cascades::ShadowCascades

// One draw's own data (#1616), mirroring `DrawUniform`
// (src/render/gpu/uniforms/material.rs) byte-for-byte: its transform on top of each
// instance's (identity for instanced solids), where its joint 0 sits in `bones`
// (#455; 0 is the shared identity), and its record in `materials`.
struct DrawUniforms {
    model_matrix: mat4x4<f32>,
    bone_base: u32,
    material: u32,
    draw_pad_a: u32,
    draw_pad_b: u32,
};

// One material record (#1616), mirroring `MaterialData`
// (src/render/gpu/uniforms/material.rs) byte-for-byte, its `SurfaceUniform` and
// `MappingUniform` written out flat. Read through `material()`. A new field takes a
// pad word or a 16-byte run at the end; that file's doc says how.
struct MaterialData {
    color_tint: vec4<f32>,
    use_texture: u32,
    is_lit: u32,
    metallic: f32,
    roughness: f32,
    // Map flags (#202, #207). metallic/roughness scale their scalar by the sampled
    // channel (glTF's blue/green at 1, red at 2: a one-channel BC4 upload, #1574);
    // normal perturbs the shading normal (2: BC5, its spread in `normal_variance`);
    // emissive modulates the emissive factor.
    use_metallic_map: u32,
    use_roughness_map: u32,
    use_normal_map: u32,
    use_emissive_map: u32,
    // Flat emissive factor (#222): rgb glow added after lighting in `fs_main`; the
    // 4th lane is unused.
    emissive: vec4<f32>,
    // Cutout alpha-test (#242): `use_cutout == 1` discards fragments whose final
    // alpha is below `alpha_cutoff`.
    use_cutout: u32,
    alpha_cutoff: f32,
    // 1 when the surface folds in its cluster's decals (#638).
    receive_decals: u32,
    // 1 when a back face may draw (#1615, render face Back or Both): `fs_main`
    // shades a face seen from behind with its normal flipped.
    two_sided: u32,
    // The transparent-surface terms (#1523). `blend_mode`: 0 Alpha, 1 Premultiply,
    // 2 Additive, 3 Multiply.
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
    // marches (`camera.ssr.x`).
    receive_ssr: u32,
    surface_pad_a: u32,
    surface_pad_b: u32,
    surface_pad_c: u32,
    // Where the maps are read (#1573): xy the tiling, zw the offset.
    uv_st: vec4<f32>,
    // 0 UV0, 1 planar (world XZ), 2 triplanar (`forward/mapping.wgsl`).
    uv_mapping: u32,
    // Texture repeats per metre under a world mapping.
    world_scale: f32,
    blend_sharpness: f32,
    mapping_pad: u32,
    // A BC5 normal map's lost spread per mip level (#1702), byte L-1 level L, read
    // when `use_normal_map == 2` (`specular_aa::table_kernel`).
    normal_variance: vec4<u32>,
};

// One object a kept scene draws (#1706), at its slot of `objects`: what the object
// holds, written when it changes, never per camera. Mirrors `ObjectData`
// (src/render/gpu/uniforms/instance.rs) byte-for-byte. Slot 0 is the identity, which
// overlays name. `use_sh == 1` reconstructs the ambient term from `sh` (#240, xyz =
// RGB radiance). Material flags stay in the draw's `MaterialData`, so texture
// sampling never depends on an object value.
struct ObjectData {
    model_matrix: mat4x4<f32>,
    use_sh: u32,
    // Baked lightmap (#438): atlas page + 1 (0 for none), and the scale/offset from
    // the mesh's lightmap UV into that page.
    lightmap_page: u32,
    // 1 when the lightmap's direction page is bound (#810): normal maps reshape
    // the baked light, and its `Baked` lamps add one specular lobe (#1695).
    lightmap_directional: u32,
    // 1 when the scene's lightmap bake took this mesh in (#1750): SSIL gathers
    // none of its emission.
    in_bake: u32,
    lightmap_st: vec4<f32>,
    sh: array<vec4<f32>, 9>,
};

// One instance of an instanced draw (#470), indexed by `instance_index`: what one
// camera decides for one drawn object this frame. Mirrors `InstanceData`
// (src/render/gpu/uniforms/instance.rs) byte-for-byte.
struct InstanceData {
    // The object's slot in `objects`.
    object: u32,
    // The resident reflection probes this instance blends (#1584): two indices plus
    // one and their weights, packed by `pack_blend`; 0 for the sky alone. Picked per
    // object at its world-bounds centre (#1206).
    reflection_probe: u32,
    // The Light Probe Proxy Volume it reads its probe light from (#1672): where the
    // volume starts in `proxy_volumes`, plus one; 0 for none.
    proxy_volume: u32,
    // Motion vectors (#1435): where last frame's world matrix sits in `bones`, its
    // last-frame palette (a skinned draw's) right after it; 0 (no history: a first
    // draw, a camera cut, a mesh particle) moves the vertex with the camera only.
    motion: u32,
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
// `reflection_probe` names. What its probes leave samples the sky's cube instead.
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

// Baked lightmap atlas pages (#438): linear light (BC6H, or Rgba16Float decoded from
// the RGBM PNG, #1694), one layer per page with its mips, read at the instance's
// page through its scale/offset; a 1x1 black page when none are baked.
@group(0) @binding(14)
var t_lightmaps: texture_2d_array<f32>;
@group(0) @binding(15)
var s_lightmaps: sampler;
// Their direction pages (#810), same layers and UVs: RGB the dominant incoming
// direction scaled by its directionality, A its lamp share (#1695;
// `decode_lightmap_direction`); a 1x1 zero page, never read, when not baked.
@group(0) @binding(16)
var t_lightmap_dirs: texture_2d_array<f32>;
// The integrated volumetric fog (#1703, `fog::volumetric`): rgb the light the air
// scatters toward the eye, a what gets through, per froxel; a 1x1x1 stand-in
// while no camera runs it, read only where `camera.volumetric.x` says so.
@group(0) @binding(17)
var t_fog_volume: texture_3d<f32>;
@group(0) @binding(18)
var s_fog_volume: sampler;

@group(1) @binding(0)
var<uniform> draw: DrawUniforms;

// Every skinned draw's joint matrices back to back (#455), sized per skin — no joint
// cap. A draw reads its run from `draw.bone_base`; element 0 is the identity.
@group(1) @binding(1)
var<storage, read> bones: array<mat4x4<f32>>;

// The camera's instances, packed in draw order; the vertex stage alone reads them
// and hands the fragment stage what it needs (`VertexOutput::instance`).
@group(1) @binding(2)
var<storage, read> instances: array<InstanceData>;

// Every kept object's record (#1706), by slot, persistent across frames and cameras;
// an instance names its own (`InstanceData::object`).
@group(1) @binding(6)
var<storage, read> objects: array<ObjectData>;

// The object instance `instance` draws.
fn object_of(instance: u32) -> u32 {
    return instances[instance].object;
}

// The frame's Light Probe Proxy Volumes (#1672), back to back; an instance reads
// its own from `proxy_volume` (layout: `forward/proxy_volume.wgsl`).
@group(1) @binding(5)
var<storage, read> proxy_volumes: array<vec4<f32>>;

// The frame's distinct material records (#1616), one per material look in use; a
// draw reads its own through `material()`. Binding 3 is the shadow casters'
// (`shader.wgsl`).
@group(1) @binding(4)
var<storage, read> materials: array<MaterialData>;

// The draw's material record. Its index is the draw's uniform, so whatever a shader
// branches on here is uniform across the draw.
fn material() -> MaterialData {
    return materials[draw.material];
}

// Where `model`'s vertex goes in world space: its skin (the draw's run of `bones`,
// #455) under the draw's transform (identity for instanced solids) over the
// instance's own. The forward pass and the selection outline (#1175) both place a
// vertex through this, so the outline cannot sit anywhere the solid does not.
fn vertex_to_world(model: VertexInput, instance: u32) -> mat4x4<f32> {
    let joints = model.joint_indices + vec4<u32>(draw.bone_base);
    let bone_transform = blend_joints(
        bones[joints.x], bones[joints.y], bones[joints.z], bones[joints.w],
        model.joint_weights,
    );
    return draw.model_matrix * objects[object_of(instance)].model_matrix * bone_transform;
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
// history, so it moves with the camera alone. Last frame's world matrix is
// `bones[motion]`; a skinned draw (`bone_base` past the identity) finds last frame's
// palette right after it.
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

// The sun's cascaded shadow maps (#435): `lighting::cascades`.
@group(3) @binding(0)
var<uniform> shadow: ShadowCascades;
@group(3) @binding(1)
var t_shadow: texture_depth_2d_array;
@group(3) @binding(2)
var s_shadow: sampler_comparison;
// Screen-space ambient occlusion (#436), one texel per pixel; a 1x1 white texture
// wherever the SSAO pass did not run (off, a stacked DepthOnly camera, the
// transparent pass).
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
// In the opaque pass of a base camera running SSIL (`camera.ssil.x`, #1750), which
// reads no opaque copy, it holds SSIL's bounce instead, one texel per pixel, so
// the stage binds no texture more than `render::setup::limits` allows (17).
@group(3) @binding(6)
var t_opaque: texture_2d<f32>;
@group(3) @binding(7)
var s_opaque: sampler;
