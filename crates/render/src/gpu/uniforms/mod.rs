//! Each `#[repr(C)]` struct here mirrors one in `engine/shaders/common.wgsl`, named apart
//! (`CameraUniform` is `CameraUniforms`, `ClusterUniform` is `Clusters`): change both together.

use crate::ibl::reflection_probes::{ReflectionProbeUniform, MAX_REFLECTION_PROBES};

mod instance;
mod material;
mod records;
mod sky;

#[cfg(test)]
mod ambient_gpu_tests;
#[cfg(test)]
mod ambient_skybox_gpu_tests;
#[cfg(test)]
mod procedural_sky_gpu_tests;

pub(crate) use instance::{InstanceData, ObjectData};
pub(crate) use material::{
    DetailUniform, DrawUniform, LevelMapsUniform, MappingUniform, MaterialData, SurfaceUniform,
    MATERIAL_DATA_WORDS,
};
pub(crate) use records::{RecordTable, RecordWords};
pub(crate) use sky::ProceduralSkyUniform;

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct CameraUniform {
    pub view_proj: [f32; 16],
    pub camera_pos: [f32; 3],
    pub time: f32,
    pub fog: FogUniform,
    pub clusters: ClusterUniform,
    pub prev_view_proj: [f32; 16],
    pub stack: [f32; 4],
    pub inv_view_proj: [f32; 16],
    pub ssr: [f32; 4],
    pub ssil: [f32; 4],
    pub lod: [f32; 4],
    pub volumetric: [f32; 4],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct ClusterUniform {
    pub view_z: [f32; 4],
    pub slices: [f32; 4],
    pub dims: [u32; 4],
}

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
    pub fn from_settings(fog: &rusty_scene::FogSettings) -> Self {
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
    pub gradient_sky: [f32; 3],
    pub mode: f32,
    pub gradient_equator: [f32; 3],
    pub _pad_equator: f32,
    pub gradient_ground: [f32; 3],
    pub _pad_ground: f32,
    pub sky_sh: [[f32; 4]; 9],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct DirectionalLightUniform {
    pub direction: [f32; 3],
    pub _pad1: f32,
    pub color: [f32; 3],
    pub intensity: f32,
    pub baked: f32,
    pub bounce_baked: f32,
    pub mask_channel: f32,
    pub _pad2: f32,
}

/// Slot 0 is the sun, the cascaded-shadow caster; more are dropped. `common.wgsl`'s
/// `array<DirectionalLight, 4>` must match.
pub(crate) const MAX_DIRECTIONAL_LIGHTS: usize = 4;

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct LightingUniform {
    // `ambient` then `sky` lead the struct: `sky_gradient.wgsl` binds that prefix alone.
    pub ambient: AmbientLightUniform,
    pub sky: ProceduralSkyUniform,
    pub dir_lights: [DirectionalLightUniform; MAX_DIRECTIONAL_LIGHTS],
    pub num_dir_lights: u32,
    pub sky_textured: f32,
    pub env_reflection: f32,
    pub diffuse_wrap: f32,
    pub baked_specular: f32,
    pub shadowmask: f32,
    pub masks_bound: f32,
    pub _pad_baked_specular: f32,
    pub refl_probes: [ReflectionProbeUniform; MAX_REFLECTION_PROBES],
}
