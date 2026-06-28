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
    pub _pad: f32,
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

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct PointLightUniform {
    pub position: [f32; 3],
    pub _pad1: f32,
    pub color: [f32; 3],
    pub intensity: f32,
    pub range: f32,
    pub _pad2: [f32; 3],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct SpotlightUniform {
    pub position: [f32; 3],
    pub _pad1: f32,
    pub direction: [f32; 3],
    pub _pad2: f32,
    pub color: [f32; 3],
    pub intensity: f32,
    pub range: f32,
    pub inner_cone: f32,
    pub outer_cone: f32,
    pub _pad3: f32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct LightingUniform {
    pub ambient: AmbientLightUniform,
    pub dir_light: DirectionalLightUniform,
    pub point_lights: [PointLightUniform; 4],
    pub spot_light: SpotlightUniform,
    pub num_point_lights: u32,
    pub ssr_active: f32,
    pub ssr_quality: f32,
    pub ssr_temporal_upsampling: f32,
    // Active reflection probe (#244): `refl_active` 1.0 when a probe's box covers the
    // camera; the shader box-projects against `[refl_box_min, refl_box_max]` around
    // `refl_center`. `refl_has_cubemap` (#245) 1.0 when that probe has a baked cube at
    // binding 4 — then the shader samples it (roughness->mip), else the skybox. Pads keep
    // each `vec4` 16-byte aligned, matching the WGSL layout.
    pub refl_active: f32,
    pub refl_has_cubemap: f32,
    pub _refl_pad: [f32; 2],
    pub refl_center: [f32; 4],
    pub refl_box_min: [f32; 4],
    pub refl_box_max: [f32; 4],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct EntityUniform {
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
    // Light-probe SH (#240). When `use_sh == 1`, the shader reconstructs ambient
    // irradiance from these 9 L2 SH coefficients — the scene's probe field
    // interpolated at this entity's position on the CPU — instead of the flat
    // hemispherical ambient term; non-static lit objects opt in. Each coefficient is
    // an RGB triple in `xyz` (4th lane unused) so the array stays `vec4`-aligned.
    pub use_sh: u32,
    // Cutout alpha-test (#242). `use_cutout == 1` makes `fs_main` discard fragments
    // whose final alpha is below `alpha_cutoff` (Cutout materials). These two scalars
    // plus `_sh_pad` complete the 16-byte run after `use_sh`, so the `sh` array that
    // follows stays `vec4`-aligned; `EntityUniforms` in shader.wgsl mirrors this field
    // order byte-for-byte.
    pub use_cutout: u32,
    pub alpha_cutoff: f32,
    pub _sh_pad: u32,
    pub sh: [[f32; 4]; 9],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct BoneUniform {
    pub bones: [[f32; 16]; 64],
}
