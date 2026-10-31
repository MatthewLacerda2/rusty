//! The opaque copy (#1523): Unity's *Opaque Texture* and *Depth Texture*, one
//! texture holding a camera's colour (rgb) and each pixel's distance from its eye
//! (alpha), drawn after its opaque pass and SSR (so a refracting surface sees the
//! reflected floor) by `opaque_copy.wgsl`. One texture, so the forward pass stays
//! inside wgpu's 16 sampled textures per stage. The transparent pass reads it
//! through its group 3. Made only on a camera whose visible translucents ask for it
//! (a material that refracts or absorbs, `MaterialAsset::reads_opaque`, or traces
//! SSR, `postfx::effects::ssr::transparent`) and counted
//! in `Debug.Stats().opaque_copies`; every other pass binds a 1x1 stand-in.
//!
//! The copy is the view's own state in `PostFx::targets`, like SSR's source copy,
//! kept while the view's size holds; a camera that needs none binds none.

use crate::render::draw::batch::DrawBatch;
use crate::render::gpu::bind_entry as entry;
use crate::render::gpu::bind_layouts::create_shadow_bind_group;
use crate::render::gpu::fullscreen::{color_pass, Fullscreen};
use crate::render::gpu::layout_entries::{depth_entry, texture_entry, uniform_entry};
use crate::render::gpu::pipelines::LazyPipeline;
use crate::render::gpu::shaders::ShaderRegistry;
use crate::render::postfx::{PfxTarget, PostFx, HDR_FORMAT};
use crate::render::timing::GpuPass;
use crate::render::{RenderView, Renderer};

/// The opaque copy a forward pass's group 3 binds, and its sampler.
#[derive(Clone, Copy)]
pub(crate) struct OpaqueViews<'a> {
    pub texture: &'a wgpu::TextureView,
    pub sampler: &'a wgpu::Sampler,
}

/// The copy's pipeline, the 1x1 stand-in every group 3 binds outside a transparent
/// pass that copied, and the copy's sampler: linear, clamped, so a lookup bent off
/// screen reads the edge.
pub(crate) struct OpaqueCopyPass {
    layout: wgpu::BindGroupLayout,
    pipeline: LazyPipeline,
    fallback: PfxTarget,
    sampler: wgpu::Sampler,
}

impl OpaqueCopyPass {
    pub(crate) fn new(device: &wgpu::Device, registry: &mut ShaderRegistry) -> Self {
        let shader = registry.load(device, "opaque_copy.wgsl", "Opaque Copy Shader");
        let d2 = wgpu::TextureViewDimension::D2;
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Opaque Copy Layout"),
            entries: &[
                uniform_entry(0, wgpu::ShaderStages::FRAGMENT),
                texture_entry(1, d2),
                depth_entry(2, d2),
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Opaque Copy Pipeline Layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = LazyPipeline::new(device, move |device| {
            let target = wgpu::ColorTargetState {
                format: HDR_FORMAT,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            };
            Fullscreen {
                label: "Opaque Copy Pipeline",
                shader: &shader,
                layout: &pipeline_layout,
                vs: "vs_fullscreen",
                fs: "fs_copy",
            }
            .colour(device, &[Some(target)])
        });
        Self {
            layout,
            pipeline,
            fallback: PostFx::target(device, 1, 1, "Opaque Copy Stand-in", HDR_FORMAT),
            sampler: device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("Opaque Copy Sampler"),
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            }),
        }
    }

    /// The stand-in's views.
    pub(crate) fn fallback(&self) -> OpaqueViews<'_> {
        OpaqueViews {
            texture: &self.fallback.view,
            sampler: &self.sampler,
        }
    }

    /// The pipeline, for warming (`Renderer::lazy_pipelines`).
    pub(crate) fn lazy_pipeline(&self) -> &LazyPipeline {
        &self.pipeline
    }
}

/// One view's opaque copy, and the group 3 its transparent pass binds this camera
/// (`None` on a camera that made no copy).
pub(crate) struct OpaqueCopy {
    size: (u32, u32),
    target: PfxTarget,
    pub(crate) group: Option<wgpu::BindGroup>,
}

/// Whether any of a camera's translucent `batches` reads the opaque copy: one that
/// refracts or absorbs, or traces SSR (#1570) on a camera that marches (`traces`).
pub(crate) fn wanted(batches: &[DrawBatch], traces: bool) -> bool {
    batches.iter().any(|b| {
        let s = b.key.record().surface;
        s.uses_opaque == 1 || (traces && s.receive_ssr == 1)
    })
}

impl Renderer {
    /// Copy this camera's opaque colour and eye distances when its `translucents`
    /// read them (`traces`: the camera marches SSR, [`wanted`]), and build the group
    /// 3 its transparent pass binds; else leave the view's copy unbound for this camera.
    pub(crate) fn copy_opaque(
        &mut self,
        view: &mut RenderView,
        translucents: &[DrawBatch],
        traces: bool,
    ) {
        if !wanted(translucents, traces) {
            if let Some(copy) = view.post_fx.targets.get_mut::<OpaqueCopy>() {
                copy.group = None;
            }
            return;
        }
        let extent = view.post_fx.scene_hdr.texture.size();
        let size = (extent.width, extent.height);
        let device = &self.device;
        let make = || OpaqueCopy {
            size,
            target: PostFx::target(device, size.0, size.1, "Opaque Copy", HDR_FORMAT),
            group: None,
        };
        let copy = view.post_fx.targets.get_or(make);
        if copy.size != size {
            *copy = make();
        }
        let copy = view
            .post_fx
            .targets
            .get::<OpaqueCopy>()
            .expect("fitted above");
        self.draw_copy(view, &copy.target.view);
        let views = OpaqueViews {
            texture: &copy.target.view,
            sampler: &self.opaque_copy.sampler,
        };
        let group = create_shadow_bind_group(
            device,
            &self.shadow_layout,
            &self.shadow_uniform_buffer,
            &self.shadow_renderer,
            &self.ssao.no_ao,
            views,
        );
        view.post_fx
            .targets
            .get_mut::<OpaqueCopy>()
            .expect("fitted above")
            .group = Some(group);
        self.frame_counters.opaque_copies += 1;
    }

    /// Draw `view`'s opaque colour and eye distances into `target`.
    fn draw_copy(&self, view: &RenderView, target: &wgpu::TextureView) {
        let pass = &self.opaque_copy;
        let view_of = |v| wgpu::BindingResource::TextureView(v);
        let inputs = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Opaque Copy Inputs"),
            layout: &pass.layout,
            entries: &[
                entry(0, self.camera_buffer.as_entire_binding()),
                entry(1, view_of(&view.post_fx.scene_hdr.view)),
                entry(2, view_of(&view.depth_view)),
            ],
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Opaque Copy Encoder"),
            });
        {
            let timed = ("Opaque Copy", &self.gpu_timer, GpuPass::Transparent);
            let clear = wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT);
            let mut rp = color_pass(&mut encoder, timed, target, clear);
            rp.set_pipeline(pass.pipeline.get());
            rp.set_bind_group(0, &inputs, &[]);
            rp.draw(0..3, 0..1);
        }
        self.queue.submit(std::iter::once(encoder.finish()));
    }

    /// The group 3 `view`'s transparent pass binds: its copy's this camera, else the
    /// no-AO group with the stand-in.
    pub(crate) fn transparent_group3<'a>(&'a self, view: &'a RenderView) -> &'a wgpu::BindGroup {
        let copy = view.post_fx.targets.get::<OpaqueCopy>();
        copy.and_then(|c| c.group.as_ref())
            .unwrap_or(&self.shadow_bind_group)
    }
}
