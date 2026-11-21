use super::super::{EffectCx, Gpu, LdrRun, Pass};
use crate::gpu::bind_entry as entry;
use crate::gpu::fullscreen::Fullscreen;
use crate::gpu::layout_entries::{texture_entry, uniform_entry};
use crate::gpu::ops::{CountedDevice as _, CountedQueue as _};
use crate::gpu::pipelines::LazyPipeline;
use crate::gpu::shaders::ShaderRegistry;
use crate::postfx::{PfxTarget, PostFx, PostFxContext};
use crate::timing::{GpuPass, GpuTimer};

pub(super) const PASS: Pass = Pass::Ldr {
    order: 15,
    prepare,
    run: LdrRun::MayEnd(run),
};

pub(crate) fn rcas_sharpness(strength: f32) -> f32 {
    let stops = (1.0 - strength.clamp(0.0, 1.0)) * 2.5;
    (-stops).exp2()
}

fn strength(cx: &EffectCx) -> f32 {
    cx.post
        .camera
        .map_or(0.0, |c| c.taa_contrast_adaptive_sharpening)
}

fn prepare(pfx: &mut PostFx, device: &wgpu::Device, cx: &EffectCx) -> bool {
    if !cx.full || strength(cx) <= 0.0 || !pfx.taa_ran() {
        pfx.targets.remove::<SharpenTargets>();
        return false;
    }
    pfx.targets.get_or(|| SharpenTargets::new(device));
    true
}

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

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct RcasParams {
    knobs: [f32; 4],
}

pub(crate) struct Sharpen {
    layout: wgpu::BindGroupLayout,
    pipelines: Vec<(wgpu::TextureFormat, LazyPipeline)>,
}

impl Sharpen {
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

    pub(crate) fn pipeline(&self, format: wgpu::TextureFormat) -> Option<&LazyPipeline> {
        let found = self.pipelines.iter().find(|(f, _)| *f == format);
        found.map(|(_, p)| p)
    }

    pub(crate) fn lazy_pipelines(&self) -> impl Iterator<Item = &LazyPipeline> {
        self.pipelines.iter().map(|(_, p)| p)
    }

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
        queue.counted_write_buffer(params, 0, bytemuck::bytes_of(&RcasParams { knobs }));
        let group = device.counted_bind_group(&wgpu::BindGroupDescriptor {
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

    pub(crate) fn params(device: &wgpu::Device) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("RCAS Params"),
            size: std::mem::size_of::<RcasParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }
}

pub(crate) struct SharpenTargets {
    params: wgpu::Buffer,
    sharpened: Option<(MadeFor, PfxTarget)>,
}

type MadeFor = ((u32, u32), wgpu::TextureFormat);

impl SharpenTargets {
    fn new(device: &wgpu::Device) -> Self {
        Self {
            params: Sharpen::params(device),
            sharpened: None,
        }
    }

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
        assert!((rcas_sharpness(1.0) - 1.0).abs() < 1e-6);
        assert!((rcas_sharpness(0.0) - (-2.5f32).exp2()).abs() < 1e-6);
        assert!(rcas_sharpness(0.75) > rcas_sharpness(0.25));
        assert_eq!(rcas_sharpness(2.0), rcas_sharpness(1.0));
    }
}
