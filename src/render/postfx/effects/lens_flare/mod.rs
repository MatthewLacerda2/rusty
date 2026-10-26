//! Lens flare (#1443), Unity URP's *Screen Space Lens Flare*: a scene pass after
//! bloom's, built from bloom's blurred bright pass, so every bright pixel flares
//! with no per-light authoring (`postfx/lens_flare.wgsl`). It draws ghosts mirrored
//! through the screen centre, a halo ring and anamorphic streaks at half the bloom
//! buffer's size (a quarter of the view's). The composite adds them after bloom's add
//! (`postfx/effects/lens_flare.wgsl`), reading the flare as its composite texture
//! (#1473), so they go through the exposure and the grade like the scene, as URP's
//! uber pass adds its flare with bloom, and SSR never reflects them.
//!
//! It runs only while bloom runs, since it reads bloom's chain, and is off on Low.
//! While it doesn't run its target is dropped and the composite reads black; with
//! no bright pixel the flare is exactly zero: either way the add changes nothing.

use super::{bloom, Composite, CompositeTexture, EffectCx, Owner, Pass, PassCx, Phase, PostEffect};
use crate::components::rendering::post_processing::LensFlare;
use crate::core::settings::graphics_options::GraphicsOption;
use crate::core::settings::quality::QualityPreset;
use crate::render::gpu::bind_entry as entry;
use crate::render::gpu::fullscreen::Fullscreen;
use crate::render::gpu::layout_entries::{sampler_entry, texture_entry, uniform_entry};
use crate::render::gpu::pipelines::LazyPipeline;
use crate::render::gpu::shaders::ShaderRegistry;
use crate::render::postfx::params::Gate;
use crate::render::postfx::{PfxTarget, PostFx, HDR_FORMAT};
use crate::render::timing::GpuPass;
use crate::render::{RenderView, Renderer};

pub(crate) const EFFECT: PostEffect = PostEffect {
    name: "lens_flare",
    owner: Owner::Volume,
    gate: Gate {
        option: Some(GraphicsOption::LensFlare),
        tiers: |q| q != QualityPreset::Low,
    },
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
    pass: Some(Pass::Scene { order: 50, run }),
};

/// w: `1` while the view flares, so the composite skips the fetch otherwise (the
/// aspect doesn't decide whether it flares).
fn pack(cx: &EffectCx, slots: &mut [[f32; 4]]) {
    let on = flare_params(cx, 1.0).is_some();
    slots[0] = [0.0, 0.0, 0.0, if on { 1.0 } else { 0.0 }];
}

/// The flare `cx`'s volume draws this frame: only while bloom runs, whose chain it
/// reads, and while some part of it can add light (`None` costs no target and no
/// pass).
pub(crate) fn flare_params(cx: &EffectCx, aspect: f32) -> Option<FlareParams> {
    let vc = cx.volume()?;
    let (b, f) = (&vc.bloom, &vc.lens_flare);
    let blooms = cx.runs(bloom::EFFECT.gate, b.active);
    if !blooms || !cx.runs(EFFECT.gate, f.active) {
        return None;
    }
    // `max` takes a NaN from a hand-edited file to zero, which draws nothing.
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

/// CPU mirror of `FlareParams` in `postfx/lens_flare.wgsl`.
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct FlareParams {
    /// rgb: the tint times the intensity, w: the ghost count.
    tint: [f32; 4],
    /// The ghosts', the halo's and the streaks' strengths, and the aberration.
    strengths: [f32; 4],
    /// x: the view's aspect, the rest unused.
    frame: [f32; 4],
}

/// The flare pass, shared by every view.
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

    /// The pipeline, for warming (`Renderer::lazy_pipelines`).
    pub(crate) fn lazy_pipelines(&self) -> [&LazyPipeline; 1] {
        [&self.flare]
    }
}

/// One view's flare target, half the bloom buffer's size, and its params. Held only
/// while the view flares.
pub(crate) struct FlareTargets {
    /// The bloom buffer's size these were made for.
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

/// The flare's size for a bloom buffer of `(w, h)`: half of it, so a quarter of
/// the view's on Medium and High (bloom's tier divisor is 2 there).
pub(crate) fn flare_size((w, h): (u32, u32)) -> (u32, u32) {
    ((w / 2).max(1), (h / 2).max(1))
}

/// The flare (bloom's blur -> its target), which the composite adds, when the view
/// flares; its target dropped when it doesn't.
fn run(r: &Renderer, view: &mut RenderView, cx: &PassCx) {
    let pfx = &mut view.post_fx;
    let bloom = pfx.bloom.size;
    let aspect = pfx.full_size.0 as f32 / pfx.full_size.1.max(1) as f32;
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
    queue.write_buffer(&t.params, 0, bytemuck::bytes_of(&params));
    let passes = &r.post_passes.lens_flare;
    let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Lens Flare Bind Group"),
        layout: &passes.layout,
        entries: &[
            entry(0, t.params.as_entire_binding()),
            entry(1, wgpu::BindingResource::TextureView(&pfx.bloom.b.view)),
            entry(2, wgpu::BindingResource::Sampler(&pfx.sampler)),
        ],
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Lens Flare Encoder"),
    });
    let flare = (passes.flare.get(), r.gpu_timer.writes(GpuPass::LensFlare));
    PostFx::fullscreen(&mut encoder, flare, &[&group], &t.flare.view);
    queue.submit(std::iter::once(encoder.finish()));
}

#[cfg(test)]
mod gpu_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod texture_gpu_tests;
