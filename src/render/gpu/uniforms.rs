//! GPU uniform memory layouts for the forward renderer, split out of `mod.rs` to keep it
//! under the size cap. Each `#[repr(C)]` struct mirrors a `struct` in `shader.wgsl`
//! byte-for-byte; the comments call out the alignment padding that keeps the two in sync.
//! `pub(crate)` so the render submodules that build and write these uniforms can name them
//! and their fields, exactly as when they lived in `mod.rs` (behavior unchanged).

use crate::render::ibl::reflection_probes::{ReflectionProbeUniform, MAX_REFLECTION_PROBES};

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct CameraUniform {
    pub view_proj: [f32; 16],
    pub camera_pos: [f32; 3],
    /// Game time for shader animation (#398), `camera.time` in WGSL — the vec3 pad slot.
    pub time: f32,
    pub fog: FogUniform,
    /// This camera's light-cluster grid (#434): how a point finds its cluster.
    pub clusters: ClusterUniform,
    /// This camera's view-projection last frame (#1435), what the velocity target
    /// reprojects through; `view_proj` itself on its first frame and after a cut.
    pub prev_view_proj: [f32; 16],
    /// `x`: this camera's index in its stack, bottom to top (#1436): the velocity
    /// target's `.a`, so motion blur orders a higher camera in front. `y`: last
    /// frame's game time (#1524), for the vertex blocks' motion. `zw` unused.
    pub stack: [f32; 4],
    /// `view_proj`'s inverse (#1523), what refraction and absorption rebuild the
    /// opaque depth's world point with.
    pub inv_view_proj: [f32; 16],
}

/// How a world point finds its light cluster (#434), mirroring `Clusters` in
/// `common.wgsl`. Built per camera by `render::clusters::ClusterGrid`.
#[repr(C)]
#[derive(Copy, Clone, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct ClusterUniform {
    /// `depth = dot(xyz, p) + w`: a point's distance along the view axis.
    pub view_z: [f32; 4],
    /// Depth slice of a point: `x` scale, `y` bias, `z` 1 for log slices
    /// (perspective) or 0 for linear (orthographic), `w` the near plane.
    pub slices: [f32; 4],
    /// Tiles across, tiles up, depth slices; `w` unused.
    pub dims: [u32; 4],
}

/// The scene fog (#437), mirroring `Fog` in `common.wgsl`. Carried by every pass's
/// camera globals (forward, sky, particles, decals), so one formula fogs them all.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct FogUniform {
    pub color: [f32; 3],
    pub mode: u32,
    pub start: f32,
    pub end: f32,
    pub density: f32,
    pub height_falloff: f32,
    pub base_height: f32,
    pub _pad: [f32; 3],
}

impl FogUniform {
    /// Pack the scene's fog setting for the GPU.
    pub fn from_settings(fog: &crate::scene::FogSettings) -> Self {
        Self {
            color: fog.color.to_array(),
            mode: fog.mode.to_index(),
            start: fog.start,
            end: fog.end,
            density: fog.density,
            height_falloff: fog.height_falloff,
            base_height: fog.base_height,
            _pad: [0.0; 3],
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct AmbientLightUniform {
    pub color: [f32; 3],
    pub intensity: f32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct DirectionalLightUniform {
    pub direction: [f32; 3],
    pub _pad1: f32,
    pub color: [f32; 3],
    pub intensity: f32,
    /// 1.0 for a `Baked` light (#438): a lightmapped surface already carries its
    /// direct light, so it skips this one.
    pub baked: f32,
    pub _pad2: [f32; 3],
}

/// The most directional lights shaded at once (#434); slot 0 is the sun, the one
/// that casts the cascaded shadows. Past this they are dropped and counted.
pub(crate) const MAX_DIRECTIONAL_LIGHTS: usize = 4;

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct LightingUniform {
    pub ambient: AmbientLightUniform,
    pub dir_lights: [DirectionalLightUniform; MAX_DIRECTIONAL_LIGHTS],
    pub num_dir_lights: u32,
    // `sky_textured` (#718) 1.0 when a skybox panorama is bound: the sky cube (#1553)
    // holds its colour, else the procedural sky's terms the shader tints. The pad keeps
    // the probe array 16-byte aligned.
    pub sky_textured: f32,
    pub _sky_pad: [f32; 2],
    // The resident reflection probes (#1206): a draw's instance names one
    // (`InstanceData::reflection_probe`), and the shader box-projects against its box
    // and samples its cube's layer of the binding-4 cube array.
    pub refl_probes: [ReflectionProbeUniform; MAX_REFLECTION_PROBES],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct EntityUniform {
    // The draw's own transform, applied on top of each instance's (#470): identity for
    // instanced solids, the gizmo's placement for editor overlays.
    pub model_matrix: [f32; 16],
    pub color_tint: [f32; 4],
    pub use_texture: u32,
    pub is_lit: u32,
    pub metallic: f32,
    pub roughness: f32,
    // Map flags (#202, #207). When set, the shader samples the matching group(2)
    // texture: metallic/roughness multiply their scalar by the sampled channel;
    // `use_normal_map` perturbs the shading normal in tangent space; `use_emissive_map`
    // modulates the emissive factor. These four `u32`s fill the run up to the next
    // 16-byte boundary so the `vec4` that follows is aligned; `EntityUniforms` in
    // shader.wgsl mirrors this field order byte-for-byte.
    pub use_metallic_map: u32,
    pub use_roughness_map: u32,
    pub use_normal_map: u32,
    pub use_emissive_map: u32,
    // Flat emissive factor (#222): rgb glow added on top of the lit colour in
    // `fs_main`. The renderer draws to an HDR target with a bloom bright-pass, so
    // values >1.0 glow automatically. Stored as a `vec4` (4th lane unused) to dodge
    // WGSL's `vec3` 16-byte-alignment gotcha and keep the struct a 16-byte multiple.
    pub emissive: [f32; 4],
    // Cutout alpha-test (#242). `use_cutout == 1` makes `fs_main` discard fragments
    // whose final alpha is below `alpha_cutoff` (Cutout materials).
    pub use_cutout: u32,
    pub alpha_cutoff: f32,
    // Index of this draw's joint 0 in the frame's bone-palette storage array (#455):
    // the vertex shader reads `bones[bone_base + joint]`. `0` is the shared identity
    // matrix every non-skinned draw uses. `EntityUniforms` in shader.wgsl mirrors this
    // field order byte-for-byte.
    pub bone_base: u32,
    // `1` when this draw folds in the decals of its cluster (#638): a lit, non-
    // transparent surface whose material receives decals. Completes the 16-byte run.
    pub receive_decals: u32,
    // How the surface composites, shadows and transmits (#1523); its fields follow
    // `receive_decals` flat in `EntityUniforms`.
    pub surface: SurfaceUniform,
}

/// A draw's transparent-surface terms (#1523): blending, shadows, refraction,
/// absorption, depth postpass and thin translucency, packed from its material.
#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct SurfaceUniform {
    /// The material's `BlendMode`, by index: 3 (Multiply) tints toward white by alpha.
    pub blend_mode: u32,
    /// 1 when URP's preserve specular applies: the diffuse is premultiplied by alpha
    /// in the shader, the specular is not, and the Fresnel raises the opacity.
    pub premultiply_diffuse: u32,
    /// 0 when no shadow lands on the surface (URP's *Receive Shadows* off).
    pub receive_shadows: u32,
    /// The refraction model: 0 none, 1 planar (HDRP's Box), 2 thin.
    pub refraction_model: u32,
    pub ior: f32,
    /// The refraction model's thickness in metres (thin: HDRP's fixed 5 mm).
    pub thickness: f32,
    /// 1 when the surface composites over the opaque copy itself (refraction or
    /// absorption), so it draws with alpha 1.
    pub uses_opaque: u32,
    /// 1 when the surface writes its depth after drawing.
    pub depth_postpass: u32,
    /// The absorption coefficient per metre, rgb (HDRP's
    /// `TransmittanceColorAtDistanceToAbsorption`); `w` unused.
    pub absorption: [f32; 4],
    /// Thin translucency: rgb the transmission colour times its amount, `w` the
    /// amount (0: none).
    pub transmission: [f32; 4],
}

/// How many 32-bit words an [`EntityUniform`] holds.
pub(crate) const ENTITY_UNIFORM_WORDS: usize = std::mem::size_of::<EntityUniform>() / 4;

impl EntityUniform {
    /// The uniform as plain words — the batch key compares these bit-for-bit, so two
    /// draws share a batch only when every material scalar and flag is identical.
    pub(crate) fn words(&self) -> [u32; ENTITY_UNIFORM_WORDS] {
        let mut words = [0u32; ENTITY_UNIFORM_WORDS];
        words.copy_from_slice(bytemuck::cast_slice(bytemuck::bytes_of(self)));
        words
    }
}

/// One instance of a forward draw (#470), read by `instance_index` from the group-1
/// storage array: the entity's world matrix, its light-probe SH (#240), and where its
/// baked lightmap sits (#438).
///
/// Only what legitimately differs between copies of one mesh + material lives here.
/// Everything the fragment shader branches on before sampling a texture stays in the
/// per-draw [`EntityUniform`], so those branches stay in uniform control flow. The SH
/// branch samples nothing, so it may vary per instance.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct InstanceData {
    pub model_matrix: [f32; 16],
    // Last frame's world matrix (#1435), read only when `motion == 1`.
    pub prev_model_matrix: [f32; 16],
    // When `use_sh == 1`, the shader reconstructs ambient irradiance from these 9 L2
    // SH coefficients — the probe field interpolated at this entity's position on the
    // CPU — instead of the flat hemispherical ambient term. Each coefficient is an RGB
    // triple in `xyz` (4th lane unused) so the array stays `vec4`-aligned.
    pub use_sh: u32,
    // The instance's baked lightmap (#438): its atlas page plus one (0 for none) and
    // the scale/offset from the mesh's lightmap UV into that page (`uv * xy + zw`).
    // Per instance, so lightmapped copies of a prop still share one draw.
    pub lightmap_page: u32,
    // 1 when the lightmap's direction page is bound (#810): normal maps then
    // reshape the baked light.
    pub lightmap_directional: u32,
    // The resident reflection probe this instance reflects, plus one; 0 for none
    // (the sky). Picked per object at its world-bounds centre (#1206).
    pub reflection_probe: u32,
    pub lightmap_st: [f32; 4],
    // Motion vectors (#1435): where last frame's joint palette starts in the joint
    // array, and 1 when it and `prev_model_matrix` hold last frame's pose; 0 moves
    // the instance with the camera alone (no history, a mesh particle).
    pub prev_bone_base: u32,
    pub motion: u32,
    pub _motion_pad: [u32; 2],
    pub sh: [[f32; 4]; 9],
}

impl InstanceData {
    /// An instance that adds nothing: identity transform, no probe SH. Overlays bind
    /// a one-element array of it and carry their transform in the per-draw uniform.
    pub(crate) const IDENTITY: Self = Self {
        model_matrix: glam::Mat4::IDENTITY.to_cols_array(),
        prev_model_matrix: glam::Mat4::IDENTITY.to_cols_array(),
        use_sh: 0,
        lightmap_page: 0,
        lightmap_directional: 0,
        reflection_probe: 0,
        lightmap_st: [0.0; 4],
        prev_bone_base: 0,
        motion: 0,
        _motion_pad: [0; 2],
        sh: [[0.0; 4]; 9],
    };
}
