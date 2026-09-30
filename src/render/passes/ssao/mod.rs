//! Screen-space ambient occlusion (#436).
//!
//! Probes light a room from a few points; they cannot darken the foot of a crate or
//! the corner of a corridor. SSAO does, per pixel, from depth — and only the
//! ambient/indirect light, never the sun or a lamp, so it has to be known *before*
//! the forward shader adds those up. Each camera therefore runs, when AO is on:
//!
//! 1. a **depth prepass** of its solids (the forward vertex stage, depth only);
//! 2. the **occlusion** pass (`ssao.wgsl` `fs_ao`) at the tier's resolution;
//! 3. a depth-aware **blur** that upsamples to full resolution (`fs_blur`);
//!
//! and the forward pass reads the result at its group 3 (`t_ao`) to scale its
//! ambient and environment-reflection terms. With AO off none of this runs, and
//! group 3 carries a 1x1 white texture instead.
//!
//! Cost, per camera: one more depth-only draw of every visible solid, plus
//! `samples` depth taps per occlusion texel (Medium: 8 at half resolution — a
//! quarter of the pixels; High: 16 at full) and 25 per pixel for the blur. The frame
//! stats count both (`draw_calls`/`triangles` include the prepass; `ssao_samples`
//! the occlusion taps).

mod run;
mod setup;
mod targets;

pub(crate) use targets::SsaoTargets;

use glam::{Mat4, Vec3};

use crate::components::SsaoSettings;
use crate::core::quality::{QualityPreset, SsaoTier};
use crate::scene::Scene;

/// The occlusion and blur targets' format: one unorm channel is all AO needs.
pub(crate) const AO_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R8Unorm;

/// The renderer-wide half of SSAO: its two fullscreen pipelines, their layout, and
/// the white texture the forward pass reads where no AO ran. The size-dependent
/// targets are per view ([`SsaoTargets`]).
pub(crate) struct SsaoRenderer {
    ao_pipeline: wgpu::RenderPipeline,
    blur_pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    _no_ao_texture: wgpu::Texture,
    /// 1x1 white: "unoccluded", bound wherever the AO pass did not run.
    pub(crate) no_ao: wgpu::TextureView,
}

/// CPU mirror of `SsaoParams` in `ssao.wgsl`.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct SsaoUniform {
    inv_view_proj: [f32; 16],
    view_proj: [f32; 16],
    camera_pos: [f32; 4],
    camera_fwd: [f32; 4],
    /// radius, intensity, sample count, full-res pixels per AO texel
    params: [f32; 4],
}

/// What this frame's SSAO does, when it runs at all: the active volume's look and
/// the quality tier's cost.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct SsaoPlan {
    pub settings: SsaoSettings,
    pub tier: SsaoTier,
}

impl SsaoPlan {
    /// The plan for `scene` at `quality`: the first active visual-correction volume
    /// with AO on, on a tier that runs it. `None` — nothing runs — otherwise.
    pub(crate) fn for_scene(scene: &Scene, quality: QualityPreset) -> Option<Self> {
        let tier = quality.ssao()?;
        let settings = scene
            .world
            .ids_with_visual_correction()
            .into_iter()
            .filter(|&id| scene.world.is_active(id))
            .find_map(|id| {
                let vc = scene.world.visual_correction(id)?;
                vc.active.then_some(vc.ssao)
            })?;
        (settings.active && settings.intensity > 0.0).then_some(Self { settings, tier })
    }
}

/// One camera's SSAO inputs: the plan plus where the camera is looking.
#[derive(Clone, Copy, Debug)]
pub(crate) struct SsaoFrame {
    pub plan: SsaoPlan,
    pub view_proj: Mat4,
    pub camera_pos: Vec3,
    pub camera_forward: Vec3,
}

impl SsaoFrame {
    /// The uniform the occlusion and blur passes read.
    pub(crate) fn uniform(&self) -> SsaoUniform {
        let s = self.plan.settings;
        SsaoUniform {
            inv_view_proj: self.view_proj.inverse().to_cols_array(),
            view_proj: self.view_proj.to_cols_array(),
            camera_pos: self.camera_pos.extend(0.0).to_array(),
            camera_fwd: self.camera_forward.normalize().extend(0.0).to_array(),
            params: [
                s.radius,
                s.intensity,
                self.plan.tier.samples as f32,
                self.plan.tier.divisor as f32,
            ],
        }
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod ssao_tests;

#[cfg(test)]
#[path = "gpu_tests.rs"]
mod ssao_gpu_tests;
