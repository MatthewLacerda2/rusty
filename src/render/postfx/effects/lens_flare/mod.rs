//! Lens flare (#1443), Unity URP's *Screen Space Lens Flare*: a scene pass after
//! bloom's, built from bloom's blurred bright pass, so every bright pixel flares
//! with no per-light authoring (`postfx/lens_flare.wgsl`). It draws ghosts mirrored
//! through the screen centre, a halo ring and anamorphic streaks at half the bloom
//! buffer's size (a quarter of the view's), then adds them onto the HDR scene the
//! composite reads, so they go through the exposure and the grade like the scene.
//!
//! It runs only while bloom runs, since it reads bloom's chain, and is off on Low.
//! The flare is added in the scene rather than in the composite because the
//! composite has no binding of an effect's own: adding one would change the
//! chain's shared bind-group layout. With no bright pixel the flare is exactly zero,
//! so the add leaves the scene byte-identical.

use super::{bloom, EffectCx, Owner, Pass, PassCx, PostEffect};
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
    module: None,
    composite: &[],
    grade: &[],
    pass: Some(Pass::Scene { order: 50, run }),
};

/// The flare `cx`'s volume draws this frame: only while bloom runs, whose chain it
/// reads, and at a non-zero strength.
pub(crate) fn flare_params(cx: &EffectCx, texel: [f32; 2], aspect: f32) -> Option<FlareParams> {
    let vc = cx.volume()?;
    let (b, f) = (&vc.bloom, &vc.lens_flare);
    let blooms = cx.runs(bloom::EFFECT.gate, b.active);
    if !blooms || !cx.runs(EFFECT.gate, f.active) || !(f.intensity > 0.0) {
        return None;
    }
    let [r, g, bl] = f.tint.map(|c| c.clamp(0.0, 1.0) * f.intensity);
    let count = f.ghost_count.min(LENS_FLARE_MAX_GHOSTS) as f32;
    let ca = f.chromatic_aberration.clamp(0.0, 1.0);
    let strength = |s: f32| s.max(0.0);
    Some(FlareParams {
        tint: [r, g, bl, count],
        strengths: [
            strength(f.ghost_intensity),
            strength(f.halo_intensity),
            strength(f.streak_intensity),
            ca,
        ],
        frame: [texel[0], texel[1], aspect, 0.0],
    })
}

/// The shader's ghost loop's bound: a loaded file bypasses the op's clamp.
const LENS_FLARE_MAX_GHOSTS: u32 =
    crate::components::rendering::post_processing::LensFlare::MAX_GHOSTS;

/// CPU mirror of `FlareParams` in `postfx/lens_flare.wgsl`.
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct FlareParams {
    /// rgb: the tint times the intensity, w: the ghost count.
    tint: [f32; 4],
    /// The ghosts', the halo's and the streaks' strengths, and the aberration.
    strengths: [f32; 4],
    /// The bloom chain's texel size, the view's aspect, unused.
    frame: [f32; 4],
}

/// The flare pass and the add, shared by every view.
pub(crate) struct FlarePasses {
    layout: wgpu::BindGroupLayout,
    flare: LazyPipeline,
    add: LazyPipeline,
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
        let pass = |fs: &'static str, blend: wgpu::BlendState| {
            let (shader, layout) = (shader.clone(), pipeline_layout.clone());
            LazyPipeline::new(device, move |device| {
                let label = format!("Lens Flare {fs}");
                let target = wgpu::ColorTargetState {
                    format: HDR_FORMAT,
                    blend: Some(blend),
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
            flare: pass("fs_flare", wgpu::BlendState::REPLACE),
            add: pass("fs_add", ADD),
            layout,
        }
    }

    /// The pipelines, for warming (`Renderer::lazy_pipelines`).
    pub(crate) fn lazy_pipelines(&self) -> [&LazyPipeline; 2] {
        [&self.flare, &self.add]
    }
}

/// The add onto the scene: its colour plus the flare's, its alpha kept.
const ADD: wgpu::BlendState = wgpu::BlendState {
    color: wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::One,
        dst_factor: wgpu::BlendFactor::One,
        operation: wgpu::BlendOperation::Add,
    },
    alpha: wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::Zero,
        dst_factor: wgpu::BlendFactor::One,
        operation: wgpu::BlendOperation::Add,
    },
};

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

/// The flare (bloom's blur -> its target), then the add onto the scene, when the
/// view flares; its target dropped when it doesn't.
fn run(r: &Renderer, view: &mut RenderView, cx: &PassCx) {
    let pfx = &mut view.post_fx;
    let bloom = pfx.bloom.size;
    let texel = [1.0 / bloom.0 as f32, 1.0 / bloom.1 as f32];
    let aspect = pfx.full_size.0 as f32 / pfx.full_size.1.max(1) as f32;
    let Some(params) = flare_params(&cx.effect, texel, aspect) else {
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
    let group = |input: &wgpu::TextureView| {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Lens Flare Bind Group"),
            layout: &passes.layout,
            entries: &[
                entry(0, t.params.as_entire_binding()),
                entry(1, wgpu::BindingResource::TextureView(input)),
                entry(2, wgpu::BindingResource::Sampler(&pfx.sampler)),
            ],
        })
    };
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Lens Flare Encoder"),
    });
    let flare = (passes.flare.get(), r.gpu_timer.writes(GpuPass::PostFx));
    PostFx::fullscreen(
        &mut encoder,
        flare,
        &[&group(&pfx.bloom.b.view)],
        &t.flare.view,
    );
    let writes = r.gpu_timer.writes(GpuPass::PostFx);
    let add_group = group(&t.flare.view);
    add(
        &mut encoder,
        (passes.add.get(), writes),
        &add_group,
        pfx.scene_color(),
    );
    queue.submit(std::iter::once(encoder.finish()));
}

/// Draw `pipeline` over `scene`, keeping what it holds: the flare's add.
fn add(
    encoder: &mut wgpu::CommandEncoder,
    (pipeline, timestamp_writes): (
        &wgpu::RenderPipeline,
        Option<wgpu::RenderPassTimestampWrites>,
    ),
    group: &wgpu::BindGroup,
    scene: &wgpu::TextureView,
) {
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        multiview_mask: None,
        label: Some("Lens Flare Add"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            depth_slice: None,
            view: scene,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Load,
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        timestamp_writes,
        occlusion_query_set: None,
    });
    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, group, &[]);
    pass.draw(0..3, 0..1);
}

#[cfg(test)]
mod gpu_tests;
#[cfg(test)]
mod tests;
