//! GPU uniform memory layouts for the forward renderer, split out of `mod.rs` to keep it
//! under the size cap. Each `#[repr(C)]` struct mirrors a `struct` in the WGSL
//! byte-for-byte; the comments call out the alignment padding that keeps the two in sync.
//! `pub(crate)` so the render submodules that build and write these uniforms can name them
//! and their fields, exactly as when they lived in `mod.rs` (behavior unchanged).

use crate::render::ibl::reflection_probes::{ReflectionProbeUniform, MAX_REFLECTION_PROBES};

mod material;
mod records;
mod sky;

#[cfg(test)]
mod ambient_gpu_tests;
#[cfg(test)]
mod ambient_skybox_gpu_tests;
#[cfg(test)]
mod procedural_sky_gpu_tests;

pub(crate) use material::{
    DrawUniform, MappingUniform, MaterialData, SurfaceUniform, MATERIAL_DATA_WORDS,
};
pub(crate) use records::{RecordTable, RecordWords};
pub(crate) use sky::ProceduralSkyUniform;

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
    /// frame's game time (#1524), for the vertex blocks' motion. `zw`: the transparent
    /// SSR march (#1570), steps and step length, zero where the camera traces none.
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
    /// The `Color` mode colour (`DrawnAmbient::color`), which the procedural sky
    /// follows whatever the mode.
    pub color: [f32; 3],
    pub intensity: f32,
    /// The `Gradient` mode's sky colour (#1647); read only when `mode` is 1.
    pub gradient_sky: [f32; 3],
    /// What surfaces take: 0.0 `color`'s hemisphere, 1.0 the sky / equator /
    /// ground trilight, 2.0 the sky's SH (`Skybox`, #1675).
    pub mode: f32,
    pub gradient_equator: [f32; 3],
    pub _pad_equator: f32,
    pub gradient_ground: [f32; 3],
    pub _pad_ground: f32,
    /// `Skybox` mode's SH, intensity applied and already convolved over π
    /// (`Sh9::convolved_over_pi`): `ambient_radiance` is one dot product with the
    /// basis. Read only when `mode` is 2; `w` unused.
    pub sky_sh: [[f32; 4]; 9],
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
    // The procedural sky's settings (#1620), right after `ambient` so the sky pass
    // reads both through a prefix of this struct (`sky_gradient.wgsl`).
    pub sky: ProceduralSkyUniform,
    pub dir_lights: [DirectionalLightUniform; MAX_DIRECTIONAL_LIGHTS],
    pub num_dir_lights: u32,
    // `sky_textured` (#718) 1.0 when a skybox panorama is bound: the sky cube (#1553)
    // holds its colour, else the procedural sky's terms the shader tints. The pad keeps
    // the probe array 16-byte aligned.
    pub sky_textured: f32,
    // `env_reflection` (#1668) 1.0 when surfaces reflect the sky and the reflection
    // probes; 0.0 in a light-probe capture, which bakes no specular (as Unity's).
    pub env_reflection: f32,
    pub _sky_pad: f32,
    // The resident reflection probes (#1206): a draw's instance names the two it
    // blends (`InstanceData::reflection_probe`, #1584), and the shader box-projects
    // against each one's box and samples its cube's layer of the binding-4 cube array.
    pub refl_probes: [ReflectionProbeUniform; MAX_REFLECTION_PROBES],
}

/// One instance of a forward draw (#470), read by `instance_index` from the group-1
/// storage array: the entity's world matrix, its light-probe SH (#240), and where its
/// baked lightmap sits (#438).
///
/// Only what legitimately differs between copies of one mesh + material lives here.
/// Everything the fragment shader branches on before sampling a texture stays in the
/// draw's [`MaterialData`] record, indexed by the per-draw uniform, so those branches
/// stay in uniform control flow. The SH
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
    // The resident reflection probes this instance blends (#1584): two indices plus
    // one and their weights, packed by `reflection_probes::pack_blend`; 0 for the sky
    // alone. Picked per object at its world-bounds centre (#1206).
    pub reflection_probe: u32,
    pub lightmap_st: [f32; 4],
    // Motion vectors (#1435): where last frame's joint palette starts in the joint
    // array, and 1 when it and `prev_model_matrix` hold last frame's pose; 0 moves
    // the instance with the camera alone (no history, a mesh particle).
    pub prev_bone_base: u32,
    pub motion: u32,
    // The Light Probe Proxy Volume it reads (#1672): where it starts in the frame's
    // proxy array (`draw::proxy_volume`), plus one; 0 for none.
    pub proxy_volume: u32,
    pub _motion_pad: u32,
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
        proxy_volume: 0,
        _motion_pad: 0,
        sh: [[0.0; 4]; 9],
    };
}
