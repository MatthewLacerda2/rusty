//! Bloom: a scene pass, before the lens flare's, which reads its chain (the bright-pass
//! threshold, then a 2D gaussian blur, at the bloom divisor's reduced resolution,
//! `postfx/bloom.wgsl`), and the composite's add of the blurred result on the HDR
//! image (`postfx/effects/bloom.wgsl`). The bright pass reads the scene after depth of field
//! and motion blur, so a moving highlight blooms along its streak, as in URP and HDRP
//! (#1436). The add lights the lens dirt too (`dirt`, #1442).

mod cache;
pub(crate) mod dirt;
#[cfg(test)]
mod dirt_tests;

use std::rc::Rc;

use super::{Composite, EffectCx, Owner, Pass, PassCx, Phase, PostEffect};
use crate::core::settings::graphics_options::GraphicsOption;
use crate::render::gpu::bind_entry as entry;
use crate::render::gpu::fullscreen::Fullscreen;
use crate::render::gpu::layout_entries::{sampler_entry, texture_entry, uniform_entry};
use crate::render::gpu::pipelines::LazyPipeline;
use crate::render::gpu::shaders::ShaderRegistry;
use crate::render::postfx::params::Gate;
use crate::render::postfx::{PfxTarget, PostFx, HDR_FORMAT};
use crate::render::timing::GpuPass;
use crate::render::{GpuTexture, RenderView, Renderer};

pub(crate) const EFFECT: PostEffect = PostEffect {
    name: "bloom",
    owner: Owner::Volume,
    gate: Gate::option(GraphicsOption::Bloom),
    module: Some("postfx/effects/bloom.wgsl"),
    composite: &[Composite {
        phase: Phase::Hdr,
        order: 30,
        function: "hdr",
        slots: 1,
        pack,
    }],
    grade: &[],
    pass: Some(Pass::Scene { order: 40, run }),
};

/// Whether `cx`'s volume blooms this frame.
fn blooms(cx: &EffectCx) -> bool {
    cx.volume()
        .is_some_and(|vc| cx.runs(EFFECT.gate, vc.bloom.active))
}

/// x: the intensity, y: the dirt's, scaled by the intensity as Unity's `UberPost`
/// scales it (`0` without a dirt texture: the composite then skips the dirt), w:
/// `1` while bloom runs (the composite skips the add otherwise).
fn pack(cx: &EffectCx, slots: &mut [[f32; 4]]) {
    let Some(vc) = cx.volume() else { return };
    let on = if blooms(cx) { 1.0 } else { 0.0 };
    let b = &vc.bloom;
    let dirt = if b.dirt_texture.is_some() {
        b.intensity * b.dirt_intensity
    } else {
        0.0
    };
    slots[0] = [b.intensity, dirt, 0.0, on];
}

/// CPU mirror of `BloomParams` in `postfx/bloom.wgsl`.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct BloomParams {
    /// threshold, the blurred target's texel size (x, y), unused
    knobs: [f32; 4],
}

/// The bright pass and the blur, shared by every view, and the lens dirt textures
/// the volumes name.
pub(crate) struct BloomPasses {
    layout: wgpu::BindGroupLayout,
    bright: LazyPipeline,
    blur: LazyPipeline,
    pub(crate) dirt: cache::DirtCache,
}

impl BloomPasses {
    pub(crate) fn new(device: &wgpu::Device, registry: &mut ShaderRegistry) -> Self {
        let shader = registry.load(device, "postfx/bloom.wgsl", "Bloom Shader");
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Bloom Layout"),
            entries: &[
                uniform_entry(0, wgpu::ShaderStages::FRAGMENT),
                texture_entry(1, wgpu::TextureViewDimension::D2),
                sampler_entry(2),
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Bloom Pipeline Layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pass = |fs: &'static str| {
            let (shader, layout) = (shader.clone(), pipeline_layout.clone());
            LazyPipeline::new(device, move |device| {
                let label = format!("Bloom {fs}");
                let target = wgpu::ColorTargetState {
                    format: HDR_FORMAT,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                };
                Fullscreen {
                    label: &label,
                    shader: &shader,
                    layout: &layout,
                    vs: "vs_fullscreen",
                    fs,
                }
                .colour(device, &[Some(target)])
            })
        };
        Self {
            bright: pass("fs_bright"),
            blur: pass("fs_blur"),
            layout,
            dirt: cache::DirtCache::default(),
        }
    }

    /// The pipelines, for warming (`Renderer::lazy_pipelines`).
    pub(crate) fn lazy_pipelines(&self) -> [&LazyPipeline; 2] {
        [&self.bright, &self.blur]
    }
}

/// One view's bloom ping-pong buffers, at the bloom divisor's reduced resolution: the
/// bright pass writes `a`, the blur `b`, which the composite adds (binding 3); and
/// the lens dirt it lights (binding 10), picked by the bloom pass each frame.
pub(crate) struct BloomTargets {
    pub(crate) a: PfxTarget,
    pub(crate) b: PfxTarget,
    pub(crate) size: (u32, u32),
    params: wgpu::Buffer,
    dirt: Option<Rc<GpuTexture>>,
    no_dirt: wgpu::TextureView,
}

impl BloomTargets {
    /// The buffers for a view `width` × `height`, `1 / divisor` of its size.
    pub(crate) fn new(device: &wgpu::Device, (w, h): (u32, u32), divisor: u32) -> Self {
        let (bw, bh) = ((w / divisor).max(1), (h / divisor).max(1));
        Self {
            a: PostFx::target(device, bw, bh, "PostFX Bloom A", HDR_FORMAT),
            b: PostFx::target(device, bw, bh, "PostFX Bloom B", HDR_FORMAT),
            size: (bw, bh),
            params: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Bloom Params"),
                size: std::mem::size_of::<BloomParams>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            dirt: None,
            no_dirt: dirt::black(device),
        }
    }

    /// The lens dirt the composite reads: this frame's texture, else black.
    pub(crate) fn dirt_view(&self) -> &wgpu::TextureView {
        self.dirt.as_ref().map_or(&self.no_dirt, |t| &t.view)
    }
}

/// Bright pass (scene -> `a`), then the blur (`a` -> `b`), when the view blooms,
/// with the lens dirt its volume names for the composite.
fn run(r: &Renderer, view: &mut RenderView, cx: &PassCx) {
    let Some(vc) = cx.effect.volume().filter(|_| blooms(&cx.effect)) else {
        view.post_fx.bloom.dirt = None;
        return;
    };
    view.post_fx.bloom.dirt = r.lens_dirt(vc);
    let (device, queue) = (&r.device, &r.queue);
    let (pfx, passes) = (&view.post_fx, &r.post_passes.bloom);
    let t = &pfx.bloom;
    let texel = [1.0 / t.size.0 as f32, 1.0 / t.size.1 as f32];
    let params = BloomParams {
        knobs: [vc.bloom.threshold, texel[0], texel[1], 0.0],
    };
    queue.write_buffer(&t.params, 0, bytemuck::bytes_of(&params));
    let group = |input: &wgpu::TextureView| {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Bloom Bind Group"),
            layout: &passes.layout,
            entries: &[
                entry(0, t.params.as_entire_binding()),
                entry(1, wgpu::BindingResource::TextureView(input)),
                entry(2, wgpu::BindingResource::Sampler(&pfx.sampler)),
            ],
        })
    };
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Bloom Encoder"),
    });
    let steps = [
        (&passes.bright, group(pfx.scene_color()), &t.a),
        (&passes.blur, group(&t.a.view), &t.b),
    ];
    for (pipeline, bind_group, target) in &steps {
        let timed = (pipeline.get(), r.gpu_timer.writes(GpuPass::PostFx));
        PostFx::fullscreen(&mut encoder, timed, &[bind_group], &target.view);
    }
    queue.submit(std::iter::once(encoder.finish()));
}
