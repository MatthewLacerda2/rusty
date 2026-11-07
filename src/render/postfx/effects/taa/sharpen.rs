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
//! **Shared, warmed (the Seam).** [`Sharpen`] is the renderer's, on
//! [`super::TaaPasses`] (`Renderer::post_passes.taa.sharpen`): its shader, layout and
//! one lazy pipeline per output format a view writes (the window's and the offscreen
//! views'), which `Renderer::lazy_pipelines` lists, so a windowed renderer builds
//! them at startup and a settings menu turning the sharpen on compiles nothing
//! mid-frame. No view builds RCAS. [`Sharpen::run`] is the bare pass (LDR input,
//! output and its format, strength), so another upscaler could sharpen its own
//! output with it, in the same place in the chain.
//!
//! **Linear in, as URP.** `PostFx::ldr` is sRGB, so the pass reads linear values and
//! its write encodes them. URP feeds RCAS linear values too (`FinalPost.shader`: "the
//! input color data for this shader variant is always linearly encoded because RCAS
//! requires it"), so nothing is encoded around the lobe.
//!
//! **Last, it writes the output.** When nothing runs after it (FXAA skips on a frame
//! TAA resolved, and the authored effects run before it) it writes the chain's output
//! itself ([`LdrRun::MayEnd`]) and the chain skips its copy, so a sharpened frame
//! holds no extra full-size target. Timed under [`GpuPass::Taa`], as a step of TAA.

use super::super::{EffectCx, Gpu, LdrRun, Pass};
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
    run: LdrRun::MayEnd(run),
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
/// whose camera's strength is above 0. Holds the view's sharpen state when it runs,
/// drops it when it doesn't.
fn prepare(pfx: &mut PostFx, device: &wgpu::Device, cx: &EffectCx) -> bool {
    if !cx.full || strength(cx) <= 0.0 || !pfx.taa_ran() {
        pfx.targets.remove::<SharpenTargets>();
        return false;
    }
    pfx.targets.get_or(|| SharpenTargets::new(device));
    true
}

/// Sharpen [`PostFx::ldr`]: straight into `ctx.output` when it is the chain's `last`
/// pass (returning true), else into the view's sharpen target, swapped in as `ldr`
/// for what follows. A view in a format the renderer built no RCAS for (none the
/// engine makes) is left unsharpened.
pub(super) fn run(
    pfx: &mut PostFx,
    gpu: Gpu,
    encoder: &mut wgpu::CommandEncoder,
    ctx: &PostFxContext,
    cx: &EffectCx,
    last: bool,
) -> bool {
    let (sharpen, format) = (&ctx.passes.taa.sharpen, pfx.output_format);
    if sharpen.pipeline(format).is_none() {
        return false;
    }
    let knobs = (strength(cx), ctx.timer);
    let size = pfx.chain_size;
    let t = pfx.targets.get_mut::<SharpenTargets>();
    let t = t.expect("prepare held it");
    if last {
        // The output is the chain's size: the scene's, or the output's under TAAU.
        t.sharpened = None;
        let io = (&pfx.ldr.view, ctx.output, format);
        sharpen.run(gpu, encoder, &t.params, io, knobs);
        return true;
    }
    let made_for = (size, format);
    if t.sharpened.as_ref().is_none_or(|(at, _)| *at != made_for) {
        let target = PostFx::target(gpu.0, size.0, size.1, "RCAS Sharpened", format);
        t.sharpened = Some((made_for, target));
    }
    let (_, target) = t.sharpened.as_mut().expect("fitted above");
    let io = (&pfx.ldr.view, &target.view, format);
    sharpen.run(gpu, encoder, &t.params, io, knobs);
    std::mem::swap(&mut pfx.ldr, target);
    false
}

/// RCAS's uniform (`RcasParams` in `postfx/rcas.wgsl`): the sharpness scalar in `.x`.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct RcasParams {
    knobs: [f32; 4],
}

/// The RCAS pass, one per renderer: its layout and one lazy pipeline per output
/// format it was built for.
pub(crate) struct Sharpen {
    layout: wgpu::BindGroupLayout,
    pipelines: Vec<(wgpu::TextureFormat, LazyPipeline)>,
}

impl Sharpen {
    /// RCAS into each of `formats` (a repeat folded), compiled when first drawn or
    /// warmed.
    pub(crate) fn new(
        device: &wgpu::Device,
        registry: &mut ShaderRegistry,
        formats: &[wgpu::TextureFormat],
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
        let mut pipelines: Vec<(wgpu::TextureFormat, LazyPipeline)> = Vec::new();
        for &format in formats {
            if pipelines.iter().any(|(f, _)| *f == format) {
                continue;
            }
            let (shader, layout) = (shader.clone(), pipeline_layout.clone());
            let target = wgpu::ColorTargetState {
                format,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            };
            let pipeline = LazyPipeline::new(device, move |device| {
                Fullscreen {
                    label: "RCAS",
                    shader: &shader,
                    layout: &layout,
                    vs: "vs_fullscreen",
                    fs: "fs_rcas",
                }
                .colour(device, &[Some(target.clone())])
            });
            pipelines.push((format, pipeline));
        }
        Self { layout, pipelines }
    }

    /// The pipeline writing `format`, if this renderer built one.
    pub(crate) fn pipeline(&self, format: wgpu::TextureFormat) -> Option<&LazyPipeline> {
        let found = self.pipelines.iter().find(|(f, _)| *f == format);
        found.map(|(_, p)| p)
    }

    /// The pipelines, for warming (`Renderer::lazy_pipelines`).
    pub(crate) fn lazy_pipelines(&self) -> impl Iterator<Item = &LazyPipeline> {
        self.pipelines.iter().map(|(_, p)| p)
    }

    /// Record RCAS at `strength` (`0..1`) from `input` into `output`, an LDR image
    /// in `format` the input's size. The bare, reusable pass; the caller owns the
    /// targets, decides whether to run it, and
    /// asks [`Self::pipeline`] first: a format with none panics here.
    pub(crate) fn run(
        &self,
        (device, queue): Gpu,
        encoder: &mut wgpu::CommandEncoder,
        params: &wgpu::Buffer,
        (input, output, format): (&wgpu::TextureView, &wgpu::TextureView, wgpu::TextureFormat),
        (strength, timer): (f32, &GpuTimer),
    ) {
        let pipeline = self
            .pipeline(format)
            .expect("an RCAS pipeline for the format");
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
        let timed = (pipeline.get(), timer.writes(GpuPass::Taa));
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

/// One view's sharpen state, held only while the view sharpens: the params uniform,
/// and an LDR image at the chain's size that trades places with [`PostFx::ldr`] on a
/// frame the sharpen is not the last pass (`None` while it writes the output).
pub(crate) struct SharpenTargets {
    params: wgpu::Buffer,
    sharpened: Option<(MadeFor, PfxTarget)>,
}

/// What a sharpen target was made for: the chain's size and the output format.
type MadeFor = ((u32, u32), wgpu::TextureFormat);

impl SharpenTargets {
    fn new(device: &wgpu::Device) -> Self {
        Self {
            params: Sharpen::params(device),
            sharpened: None,
        }
    }

    /// Whether the view holds a chain-size sharpen target: it ran, but not last.
    #[cfg(test)]
    pub(crate) fn has_target(&self) -> bool {
        self.sharpened.is_some()
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
