//! src/render/postfx/effects/taa/sharpen.rs — contrast-adaptive sharpening (#1847):
//! AMD's RCAS over the TAA-resolved image, in `postfx/rcas.wgsl`. URP's TAA
//! *Contrast Adaptive Sharpening* and FSR both sharpen this way.
//!
//! **Where it runs: URP's.** URP sharpens in its final post pass
//! (`FinalPostProcessPass`), after the uber pass tonemapped the image, so TAA's
//! sharpen here is a [`Pass::Ldr`] at the chain's size (`PostFx::chain_size`, the
//! output's while TAA upscales), after the authored effects and before FXAA's slot.
//! Bloom, the exposure meter and motion blur read the unsharpened HDR scene. It runs
//! only on a frame TAA resolved, with the camera's strength above 0 ([`prepare`]); at
//! 0 the pass is never recorded and its targets are dropped, so the frame is
//! byte-identical to no sharpen.
//!
//! **Reusable (the Seam).** [`Sharpen::run`] is the bare pass — LDR input view,
//! output view, strength — held per view on [`PostFx::sharpen`], so #1848's FSR2
//! sharpens its own upscaled output with it, in the same place in the chain.
//! Timed under [`GpuPass::Taa`], as a step of TAA.

use super::super::{EffectCx, Gpu, LdrRun, Pass, Targets};
use crate::render::gpu::bind_entry as entry;
use crate::render::gpu::fullscreen::Fullscreen;
use crate::render::gpu::layout_entries::{texture_entry, uniform_entry};
use crate::render::gpu::pipelines::LazyPipeline;
use crate::render::gpu::shaders::ShaderRegistry;
use crate::render::postfx::{PfxTarget, PostFx, PostFxContext};
use crate::render::timing::{GpuPass, GpuTimer};

/// TAA's sharpen as an LDR pass: after the authored effects (10), before FXAA (20),
/// as URP's final pass sharpens after the effects injected after post-processing.
pub(super) const PASS: Pass = Pass::Ldr {
    order: 15,
    prepare,
    run: LdrRun::InPlace(run),
};

/// URP's slope from a `0..1` strength to FSR's RCAS sharpness scalar: `strength`
/// maps to `(1 - strength)` stops × URP's `kMaxSharpnessStops` (2.5), then
/// `exp2(-stops)` (`FSRUtils.SetRcasConstantsLinear`). A strength of 1 is the
/// sharpest (1.0); 0 would be 0.177 — but strength 0 never runs the pass.
pub(crate) fn rcas_sharpness(strength: f32) -> f32 {
    let stops = (1.0 - strength.clamp(0.0, 1.0)) * 2.5;
    (-stops).exp2()
}

/// The camera's sharpen strength this frame: 0 for a view without one.
fn strength(cx: &EffectCx) -> f32 {
    cx.post
        .camera
        .map_or(0.0, |c| c.taa_contrast_adaptive_sharpening)
}

/// Whether the sharpen runs: a full post-FX view whose TAA resolved this frame and
/// whose camera's strength is above 0. Fits the targets when it runs, drops them
/// when it doesn't.
fn prepare(pfx: &mut PostFx, device: &wgpu::Device, cx: &EffectCx) -> bool {
    if !cx.full || strength(cx) <= 0.0 || !pfx.taa_ran() {
        pfx.targets.remove::<SharpenTargets>();
        return false;
    }
    let (size, format) = (pfx.chain_size, pfx.output_format);
    SharpenTargets::fit(&mut pfx.targets, device, size, format);
    true
}

/// Sharpen [`PostFx::ldr`] into the view's sharpen target, then swap the two, so
/// the result is `ldr` for what follows.
fn run(
    pfx: &mut PostFx,
    gpu: Gpu,
    encoder: &mut wgpu::CommandEncoder,
    ctx: &PostFxContext,
    cx: &EffectCx,
) {
    let t = pfx
        .targets
        .get::<SharpenTargets>()
        .expect("prepare fitted it");
    let io = (&pfx.ldr.view, &t.sharpened.view);
    let knobs = (strength(cx), ctx.timer);
    pfx.sharpen.run(gpu, encoder, &t.params, io, knobs);
    let t = pfx.targets.get_mut::<SharpenTargets>().expect("held above");
    std::mem::swap(&mut pfx.ldr, &mut t.sharpened);
}

/// RCAS's uniform (`RcasParams` in `postfx/rcas.wgsl`): the sharpness scalar in `.x`.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct RcasParams {
    knobs: [f32; 4],
}

/// The RCAS pipeline, a view's ([`PostFx::sharpen`]): it writes the view's output
/// format, built the first frame it sharpens.
pub(crate) struct Sharpen {
    layout: wgpu::BindGroupLayout,
    pipeline: LazyPipeline,
}

impl Sharpen {
    pub(crate) fn new(
        device: &wgpu::Device,
        registry: &mut ShaderRegistry,
        format: wgpu::TextureFormat,
    ) -> Self {
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
            format,
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

    /// Record RCAS at `strength` (`0..1`) from `input` into `output`, both LDR
    /// images in the view's output format. The bare, reusable pass (#1848 calls it
    /// after its upscale); the caller owns the targets and decides whether to run it.
    pub(crate) fn run(
        &self,
        (device, queue): Gpu,
        encoder: &mut wgpu::CommandEncoder,
        params: &wgpu::Buffer,
        (input, output): (&wgpu::TextureView, &wgpu::TextureView),
        (strength, timer): (f32, &GpuTimer),
    ) {
        let knobs = [rcas_sharpness(strength), 0.0, 0.0, 0.0];
        queue.write_buffer(params, 0, bytemuck::bytes_of(&RcasParams { knobs }));
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

    /// A uniform buffer for [`Sharpen::run`]'s params.
    pub(crate) fn params(device: &wgpu::Device) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("RCAS Params"),
            size: std::mem::size_of::<RcasParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }
}

/// One view's sharpen target: an LDR image at the chain's size that trades places
/// with [`PostFx::ldr`] each frame it runs, and the params uniform. Held only while
/// the view sharpens.
pub(crate) struct SharpenTargets {
    made_for: ((u32, u32), wgpu::TextureFormat),
    sharpened: PfxTarget,
    params: wgpu::Buffer,
}

impl SharpenTargets {
    fn new(device: &wgpu::Device, made_for: ((u32, u32), wgpu::TextureFormat)) -> Self {
        let ((w, h), format) = made_for;
        Self {
            made_for,
            sharpened: PostFx::target(device, w, h, "RCAS Sharpened", format),
            params: Sharpen::params(device),
        }
    }

    fn fit(
        targets: &mut Targets,
        device: &wgpu::Device,
        size: (u32, u32),
        format: wgpu::TextureFormat,
    ) {
        let t = targets.get_or(|| Self::new(device, (size, format)));
        if t.made_for != (size, format) {
            *t = Self::new(device, (size, format));
        }
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
