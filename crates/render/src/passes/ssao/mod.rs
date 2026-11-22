mod run;
mod setup;
mod targets;

pub(crate) use targets::SsaoTargets;

use glam::{Mat4, Vec3};

use crate::gpu::pipelines::LazyPipeline;
use crate::postfx::params::{effect_runs, Gate};
use crate::timing::GpuPass;
use rusty_components::{PostProcessingComponent, SsaoSettings};
use rusty_core::settings::graphics_options::GraphicsOption;
use rusty_core::settings::quality::{SsaoQuality, MAX_RESOLUTION_DIVISOR};
use rusty_core::settings::GraphicsSettings;
use rusty_scene::Camera;

pub(crate) const AO_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R8Unorm;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Stage {
    Occlusion,
    Blur,
    Upsample,
}

impl Stage {
    const ALL: [Self; 3] = [Self::Occlusion, Self::Blur, Self::Upsample];

    fn label(self) -> &'static str {
        match self {
            Self::Occlusion => "SSAO Pass",
            Self::Blur => "SSAO Blur Pass",
            Self::Upsample => "SSAO Upsample Pass",
        }
    }

    fn entry(self) -> &'static str {
        match self {
            Self::Occlusion => "fs_ao",
            Self::Blur => "fs_blur",
            Self::Upsample => "fs_upsample",
        }
    }

    fn timed(self) -> GpuPass {
        match self {
            Self::Occlusion => GpuPass::Ssao,
            Self::Blur | Self::Upsample => GpuPass::SsaoBlur,
        }
    }
}

pub(crate) struct SsaoRenderer {
    pipelines: [LazyPipeline; 3],
    layout: wgpu::BindGroupLayout,
    _no_ao_texture: wgpu::Texture,
    pub(crate) no_ao: wgpu::TextureView,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct SsaoUniform {
    inv_view_proj: [f32; 16],
    view_proj: [f32; 16],
    camera_pos: [f32; 4],
    camera_fwd: [f32; 4],
    params: [f32; 4],
}

const GATE: Gate = Gate::option(GraphicsOption::Ssao);

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct SsaoPlan {
    pub settings: SsaoSettings,
    pub quality: SsaoQuality,
}

impl SsaoPlan {
    pub(crate) fn for_volume(
        volume: Option<&PostProcessingComponent>,
        graphics: GraphicsSettings,
    ) -> Option<Self> {
        let settings = volume?.ssao;
        let on = settings.active && settings.intensity > 0.0;
        if !effect_runs(graphics, GATE, on) {
            return None;
        }
        let custom = SsaoQuality {
            samples: settings.samples,
            resolution_divisor: settings.resolution_divisor,
        };
        let picked = settings.quality.pick(&graphics.quality.ssao, custom);
        let quality = SsaoQuality {
            samples: picked.samples.clamp(1, SsaoSettings::MAX_SAMPLES),
            resolution_divisor: picked.resolution_divisor.clamp(1, MAX_RESOLUTION_DIVISOR),
        };
        Some(Self { settings, quality })
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct SsaoFrame {
    pub plan: SsaoPlan,
    pub view_proj: Mat4,
    pub camera_pos: Vec3,
    pub camera_forward: Vec3,
}

impl SsaoFrame {
    pub(crate) fn for_camera(plan: SsaoPlan, view_proj: Mat4, camera: &Camera) -> Self {
        Self {
            plan,
            view_proj,
            camera_pos: camera.position,
            camera_forward: camera.forward(),
        }
    }

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
                self.plan.quality.samples as f32,
                self.plan.quality.resolution_divisor as f32,
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

#[cfg(test)]
#[path = "gpu_upsample_tests.rs"]
mod ssao_gpu_upsample_tests;
