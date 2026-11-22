use super::{
    bloom, Composite, CompositeTexture, EffectCx, History, Owner, Pass, PassCx, Phase, PostEffect,
};
use crate::gpu::bind_entry as entry;
use crate::gpu::commands::Commands;
use crate::gpu::fullscreen::Fullscreen;
use crate::gpu::layout_entries::{sampler_entry, texture_entry, uniform_entry};
use crate::gpu::ops::{CountedDevice as _, CountedQueue as _};
use crate::gpu::pipelines::LazyPipeline;
use crate::gpu::shaders::ShaderRegistry;
use crate::postfx::params::Gate;
use crate::postfx::{PfxTarget, PostFx, HDR_FORMAT};
use crate::timing::GpuPass;
use crate::{RenderView, Renderer};
use rusty_components::rendering::post_processing::LensFlare;
use rusty_core::settings::graphics_options::GraphicsOption;

pub(crate) const EFFECT: PostEffect = PostEffect {
    name: "lens_flare",
    owner: Owner::Volume,
    gate: Gate::option(GraphicsOption::LensFlare),
    history: History::None("drawn from this frame's bright pixels"),
    module: Some("postfx/effects/lens_flare.wgsl"),
    composite: &[Composite {
        phase: Phase::Hdr,
        // After bloom's add (30), before the exposure (40).
        order: 35,
        function: "hdr",
        slots: 1,
        textures: &[CompositeTexture {
            name: "image",
            view: |pfx| pfx.targets.get::<FlareTargets>().map(|t| &t.flare.view),
        }],
        pack,
    }],
    grade: &[],
    passes: &[Pass::Scene { order: 50, run }],
};

fn pack(cx: &EffectCx, slots: &mut [[f32; 4]]) {
    let on = flare_params(cx, 1.0).is_some();
    slots[0] = [0.0, 0.0, 0.0, if on { 1.0 } else { 0.0 }];
}

pub(crate) fn flare_params(cx: &EffectCx, aspect: f32) -> Option<FlareParams> {
    let vc = cx.volume()?;
    let (b, f) = (&vc.bloom, &vc.lens_flare);
    let blooms = cx.runs(bloom::EFFECT.gate, b.active);
    if !blooms || !cx.runs(EFFECT.gate, f.active) {
        return None;
    }
    let strength = |s: f32| s.max(0.0);
    let intensity = strength(f.intensity);
    let tint = f.tint.map(|c| c.clamp(0.0, 1.0) * intensity);
    let count = f.ghost_count.min(LensFlare::MAX_GHOSTS);
    let ghosts = if count == 0 {
        0.0
    } else {
        strength(f.ghost_intensity)
    };
    let parts = [
        ghosts,
        strength(f.halo_intensity),
        strength(f.streak_intensity),
    ];
    if tint == [0.0; 3] || parts == [0.0; 3] {
        return None;
    }
    let [r, g, bl] = tint;
    Some(FlareParams {
        tint: [r, g, bl, count as f32],
        strengths: [
            parts[0],
            parts[1],
            parts[2],
            f.chromatic_aberration.clamp(0.0, 1.0),
        ],
        frame: [aspect, 0.0, 0.0, 0.0],
    })
}

/// Mirrors `FlareParams` in `postfx/lens_flare.wgsl`.
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct FlareParams {
    tint: [f32; 4],
    strengths: [f32; 4],
    frame: [f32; 4],
}

pub(crate) struct FlarePasses {
    layout: wgpu::BindGroupLayout,
    flare: LazyPipeline,
}

impl FlarePasses {
    pub(crate) fn new(device: &wgpu::Device, registry: &mut ShaderRegistry) -> Self {
        let shader = registry.load(device, "postfx/lens_flare.wgsl", "Lens Flare Shader");
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Lens Flare Layout"),
            entries: &[
                uniform_entry(0, wgpu::ShaderStages::FRAGMENT),
                texture_entry(1, wgpu::TextureViewDimension::D2),
                sampler_entry(2),
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Lens Flare Pipeline Layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let flare = LazyPipeline::new(device, move |device| {
            let target = wgpu::ColorTargetState {
                format: HDR_FORMAT,
                blend: Some(wgpu::BlendState::REPLACE),
                write_mask: wgpu::ColorWrites::ALL,
            };
            Fullscreen {
                label: "Lens Flare fs_flare",
                shader: &shader,
                layout: &pipeline_layout,
                vs: "vs_fullscreen",
                fs: "fs_flare",
            }
            .colour(device, &[Some(target)])
        });
        Self { flare, layout }
    }

    pub(crate) fn lazy_pipelines(&self) -> [&LazyPipeline; 1] {
        [&self.flare]
    }
}

pub(crate) struct FlareTargets {
    bloom: (u32, u32),
    pub(crate) flare: PfxTarget,
    params: wgpu::Buffer,
}

impl FlareTargets {
    fn new(device: &wgpu::Device, bloom: (u32, u32)) -> Self {
        let (w, h) = flare_size(bloom);
        Self {
            bloom,
            flare: PostFx::target(device, w, h, "PostFX Lens Flare", HDR_FORMAT),
            params: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Lens Flare Params"),
                size: std::mem::size_of::<FlareParams>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
        }
    }
}

pub(crate) fn flare_size((w, h): (u32, u32)) -> (u32, u32) {
    ((w / 2).max(1), (h / 2).max(1))
}

fn run(r: &Renderer, view: &mut RenderView, cx: &PassCx, commands: &mut Commands) {
    let pfx = &mut view.post_fx;
    let Some(bloom) = pfx.targets.get::<bloom::BloomTargets>().map(|t| t.size) else {
        pfx.targets.remove::<FlareTargets>();
        return;
    };
    let aspect = pfx.chain_size.0 as f32 / pfx.chain_size.1.max(1) as f32;
    let Some(params) = flare_params(&cx.effect, aspect) else {
        pfx.targets.remove::<FlareTargets>();
        return;
    };
    let (device, queue) = (&r.device, &r.queue);
    let t = pfx.targets.get_or(|| FlareTargets::new(device, bloom));
    if t.bloom != bloom {
        *t = FlareTargets::new(device, bloom);
    }
    let pfx = &view.post_fx;
    let t = pfx.targets.get::<FlareTargets>().expect("made above");
    let chain = pfx
        .targets
        .get::<bloom::BloomTargets>()
        .expect("checked above");
    queue.counted_write_buffer(&t.params, 0, bytemuck::bytes_of(&params));
    let passes = &r.post_passes.lens_flare;
    let group = device.counted_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Lens Flare Bind Group"),
        layout: &passes.layout,
        entries: &[
            entry(0, t.params.as_entire_binding()),
            entry(1, wgpu::BindingResource::TextureView(chain.prefiltered())),
            entry(2, wgpu::BindingResource::Sampler(&pfx.sampler)),
        ],
    });
    let flare = (passes.flare.get(), r.gpu_timer.writes(GpuPass::LensFlare));
    PostFx::fullscreen(commands.encoder(), flare, &[&group], &t.flare.view);
}

#[cfg(test)]
mod gpu_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod texture_gpu_tests;
