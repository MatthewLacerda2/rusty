//! GPU uniform memory layouts for the forward renderer, split out of `mod.rs` to keep it
//! under the size cap. Each `#[repr(C)]` struct mirrors a `struct` in `shader.wgsl`
//! byte-for-byte; the comments call out the alignment padding that keeps the two in sync.
//! `pub(crate)` so the render submodules that build and write these uniforms can name them
//! and their fields, exactly as when they lived in `mod.rs` (behavior unchanged).

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
    pub _pad2: [f32; 4],
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
    pub ssr_active: f32,
    pub ssr_quality: f32,
    pub ssr_temporal_upsampling: f32,
    // Active reflection probe (#244): `refl_active` 1.0 when a probe's box covers the
    // camera; the shader box-projects against `[refl_box_min, refl_box_max]` around
    // `refl_center`. `refl_has_cubemap` (#245) 1.0 when that probe has a baked cube at
    // binding 4 — then the shader samples it (roughness->mip), else the skybox. Pads keep
    // each `vec4` 16-byte aligned, matching the WGSL layout. `sky_textured` (#718) 1.0
    // when a skybox panorama is bound, else the shader reflects the procedural sky.
    pub refl_active: f32,
    pub refl_has_cubemap: f32,
    pub sky_textured: f32,
    pub _refl_pad: f32,
    pub refl_center: [f32; 4],
    pub refl_box_min: [f32; 4],
    pub refl_box_max: [f32; 4],
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
    // matrix every non-skinned draw uses. The pad completes the 16-byte run;
    // `EntityUniforms` in shader.wgsl mirrors this field order byte-for-byte.
    pub bone_base: u32,
    pub _pad: u32,
}

impl EntityUniform {
    /// The uniform as plain words — the batch key compares these bit-for-bit, so two
    /// draws share a batch only when every material scalar and flag is identical.
    pub(crate) fn words(&self) -> [u32; 36] {
        let mut words = [0u32; 36];
        words.copy_from_slice(bytemuck::cast_slice(bytemuck::bytes_of(self)));
        words
    }
}

/// One instance of a forward draw (#470), read by `instance_index` from the group-1
/// storage array: the entity's world matrix, and its light-probe SH (#240).
///
/// Only what legitimately differs between copies of one mesh + material lives here.
/// Everything the fragment shader branches on before sampling a texture stays in the
/// per-draw [`EntityUniform`], so those branches stay in uniform control flow. The SH
/// branch samples nothing, so it may vary per instance.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct InstanceData {
    pub model_matrix: [f32; 16],
    // When `use_sh == 1`, the shader reconstructs ambient irradiance from these 9 L2
    // SH coefficients — the probe field interpolated at this entity's position on the
    // CPU — instead of the flat hemispherical ambient term. Each coefficient is an RGB
    // triple in `xyz` (4th lane unused) so the array stays `vec4`-aligned.
    pub use_sh: u32,
    pub _pad: [u32; 3],
    pub sh: [[f32; 4]; 9],
}

impl InstanceData {
    /// An instance that adds nothing: identity transform, no probe SH. Overlays bind
    /// a one-element array of it and carry their transform in the per-draw uniform.
    pub(crate) const IDENTITY: Self = Self {
        model_matrix: glam::Mat4::IDENTITY.to_cols_array(),
        use_sh: 0,
        _pad: [0; 3],
        sh: [[0.0; 4]; 9],
    };
}
