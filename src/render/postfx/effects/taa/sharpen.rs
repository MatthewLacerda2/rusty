//! src/render/postfx/effects/taa/sharpen.rs — contrast-adaptive sharpening (#1847):
//! AMD's RCAS over the TAA resolve, in `postfx/rcas.wgsl`. URP's TAA *Contrast
//! Adaptive Sharpening* and FSR both sharpen this way after their resolve.
//!
//! One fullscreen pass at the chain's size (`PostFx::chain_size`), the output's
//! while TAA upscales. It runs only when the camera's strength is above 0
//! ([`PostFx::run_sharpen`]); at 0 the targets are dropped and the frame stays
//! byte-identical. Its result becomes the resolved image the rest of the chain
//! reads (`PostFx::scene_color` through `scene_of`), so motion blur, bloom and the
//! composite see the sharpened scene.
//!
//! **Reusable (the Seam).** [`Sharpen::run`] is the bare pass — input view, output
//! view, strength — so #1848's FSR2 can sharpen its own upscaled output with it.
//! [`PostFx::run_sharpen`] is the TAA wiring around it (the target, the gate).
//! Timed under [`GpuPass::Taa`], as a step of the resolve.

use super::super::Targets;
use crate::render::gpu::bind_entry as entry;
use crate::render::gpu::fullscreen::Fullscreen;
use crate::render::gpu::layout_entries::{texture_entry, uniform_entry};
use crate::render::gpu::pipelines::LazyPipeline;
use crate::render::gpu::shaders::ShaderRegistry;
use crate::render::postfx::{PfxTarget, PostFx, HDR_FORMAT};
use crate::render::timing::{GpuPass, GpuTimer};

/// URP's slope from a `0..1` strength to FSR's RCAS sharpness scalar: `strength`
/// maps to `(1 - strength)` stops × URP's `kMaxSharpnessStops` (2.5), then
/// `exp2(-stops)` (`FSRUtils.SetRcasConstantsLinear`). A strength of 1 is the
/// sharpest (1.0); 0 would be 0.177 — but strength 0 never runs the pass.
pub(crate) fn rcas_sharpness(strength: f32) -> f32 {
    let stops = (1.0 - strength.clamp(0.0, 1.0)) * 2.5;
    (-stops).exp2()
}

/// RCAS's uniform (`RcasParams` in `postfx/rcas.wgsl`): the sharpness scalar in `.x`.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct RcasParams {
    knobs: [f32; 4],
}

/// The RCAS pipeline, shared by every view (and reusable by #1848).
pub(crate) struct Sharpen {
    layout: wgpu::BindGroupLayout,
    pipeline: LazyPipeline,
}

impl Sharpen {
    pub(crate) fn new(device: &wgpu::Device, registry: &mut ShaderRegistry) -> Self {
        let shader = registry.load(device, "postfx/rcas.wgsl", "RCAS Shader");
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("RCAS Layout"),
            entries: &[
                uniform_entry(0, wgpu::ShaderStages::FRAGMENT),
                texture_entry(1, wgpu::TextureViewDimension::D2),
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("RCAS Pipeline Layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let target = wgpu::ColorTargetState {
            format: HDR_FORMAT,
            blend: None,
            write_mask: wgpu::ColorWrites::ALL,
        };
        let pipeline = LazyPipeline::new(device, move |device| {
            Fullscreen {
                label: "RCAS",
                shader: &shader,
                layout: &pipeline_layout,
                vs: "vs_fullscreen",
                fs: "fs_rcas",
            }
            .colour(device, &[Some(target.clone())])
        });
        Self { layout, pipeline }
    }

    /// Sharpen `input` into `output` with RCAS at `strength` (`0..1`). The bare,
    /// reusable pass (#1848 calls it after its upscale); the caller owns the
    /// targets and decides whether to run it.
    pub(crate) fn run(
        &self,
        (device, queue): (&wgpu::Device, &wgpu::Queue),
        encoder: &mut wgpu::CommandEncoder,
        params: &wgpu::Buffer,
        (input, output): (&wgpu::TextureView, &wgpu::TextureView),
        (strength, timer): (f32, &GpuTimer),
    ) {
        queue.write_buffer(
            params,
            0,
            bytemuck::bytes_of(&RcasParams {
                knobs: [rcas_sharpness(strength), 0.0, 0.0, 0.0],
            }),
        );
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("RCAS Bind Group"),
            layout: &self.layout,
            entries: &[
                entry(0, params.as_entire_binding()),
                entry(1, wgpu::BindingResource::TextureView(input)),
            ],
        });
        let timed = (self.pipeline.get(), timer.writes(GpuPass::Taa));
        PostFx::fullscreen(encoder, timed, &[&group], output);
    }

    /// The pipeline, for warming (`Renderer::lazy_pipelines`).
    pub(crate) fn lazy_pipelines(&self) -> [&LazyPipeline; 1] {
        [&self.pipeline]
    }
}

/// One view's RCAS target: the sharpened scene and its params uniform, at the
/// chain's size. Held only while the camera sharpens (strength above 0).
pub(crate) struct SharpenTargets {
    size: (u32, u32),
    sharpened: PfxTarget,
    params: wgpu::Buffer,
}

impl SharpenTargets {
    /// The sharpened scene the rest of the chain reads (`scene_of`).
    pub(crate) fn sharpened_view(&self) -> &wgpu::TextureView {
        &self.sharpened.view
    }

    fn new(device: &wgpu::Device, (w, h): (u32, u32)) -> Self {
        Self {
            size: (w, h),
            sharpened: PostFx::target(device, w, h, "RCAS Sharpened", HDR_FORMAT),
            params: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("RCAS Params"),
                size: std::mem::size_of::<RcasParams>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
        }
    }

    fn fit(targets: &mut Targets, device: &wgpu::Device, size: (u32, u32)) {
        let t = targets.get_or(|| Self::new(device, size));
        if t.size != size {
            *t = Self::new(device, size);
        }
    }
}

impl PostFx {
    /// Sharpen this frame's resolve with RCAS at `strength` (#1847), or drop the
    /// targets when `strength` is 0 or TAA did not resolve this frame — then the
    /// frame is byte-identical to no sharpen. Afterwards [`PostFx::scene_color`]
    /// (and motion blur's input) read the sharpened image.
    pub(crate) fn run_sharpen(
        &mut self,
        (device, queue): (&wgpu::Device, &wgpu::Queue),
        sharpen: &Sharpen,
        timer: &GpuTimer,
        strength: f32,
    ) {
        if strength <= 0.0 || !self.taa_ran() {
            self.targets.remove::<SharpenTargets>();
            return;
        }
        SharpenTargets::fit(&mut self.targets, device, self.chain_size);
        let input = self.resolved_color();
        let targets = self.targets.get::<SharpenTargets>().expect("fitted above");
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("RCAS Encoder"),
        });
        sharpen.run(
            (device, queue),
            &mut encoder,
            &targets.params,
            (input, &targets.sharpened.view),
            (strength, timer),
        );
        queue.submit(std::iter::once(encoder.finish()));
    }
}

#[cfg(test)]
mod tests {
    use super::rcas_sharpness;

    #[test]
    fn strength_maps_to_fsrs_sharpness_scalar() {
        // Strength 1 is the sharpest (FSR's 1.0); the slope is URP's 2.5 stops.
        assert!((rcas_sharpness(1.0) - 1.0).abs() < 1e-6);
        assert!((rcas_sharpness(0.0) - (-2.5f32).exp2()).abs() < 1e-6);
        // Monotonic: more strength, more sharpening.
        assert!(rcas_sharpness(0.75) > rcas_sharpness(0.25));
        // Out-of-range strengths clamp.
        assert_eq!(rcas_sharpness(2.0), rcas_sharpness(1.0));
    }
}
