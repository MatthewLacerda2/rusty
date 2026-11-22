use crate::draw::batch::DrawBatch;
use crate::gpu::bind_entry as entry;
use crate::gpu::bind_layouts::create_shadow_bind_group;
use crate::gpu::fullscreen::{color_pass, Fullscreen};
use crate::gpu::layout_entries::{depth_entry, texture_entry, uniform_entry};
use crate::gpu::ops::CountedDevice as _;
use crate::gpu::pipelines::LazyPipeline;
use crate::gpu::shaders::ShaderRegistry;
use crate::postfx::{PfxTarget, PostFx, HDR_FORMAT};
use crate::timing::GpuPass;
use crate::{RenderView, Renderer};

#[derive(Clone, Copy)]
pub(crate) struct OpaqueViews<'a> {
    pub texture: &'a wgpu::TextureView,
    pub sampler: &'a wgpu::Sampler,
}

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
                // Clamped: a lookup bent off screen reads the edge.
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            }),
        }
    }

    pub(crate) fn fallback(&self) -> OpaqueViews<'_> {
        OpaqueViews {
            texture: &self.fallback.view,
            sampler: &self.sampler,
        }
    }

    pub(crate) fn lazy_pipeline(&self) -> &LazyPipeline {
        &self.pipeline
    }
}

pub(crate) struct OpaqueCopy {
    size: (u32, u32),
    target: PfxTarget,
    pub(crate) group: Option<wgpu::BindGroup>,
}

pub(crate) fn wanted(batches: &[DrawBatch], traces: bool) -> bool {
    batches.iter().any(|b| {
        let t = b.key.traits;
        t.uses_opaque || (traces && t.receive_ssr)
    })
}

impl Renderer {
    pub(crate) fn copy_opaque(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
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
        self.draw_copy(encoder, view, &copy.target.view);
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

    fn draw_copy(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        view: &RenderView,
        target: &wgpu::TextureView,
    ) {
        let pass = &self.opaque_copy;
        let view_of = |v| wgpu::BindingResource::TextureView(v);
        let inputs = self.device.counted_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Opaque Copy Inputs"),
            layout: &pass.layout,
            entries: &[
                entry(0, self.camera_buffer.uniform.as_entire_binding()),
                entry(1, view_of(&view.post_fx.scene_hdr.view)),
                entry(2, view_of(&view.depth_view)),
            ],
        });
        {
            let timed = ("Opaque Copy", &self.gpu_timer, GpuPass::Transparent);
            let clear = wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT);
            let mut rp = color_pass(encoder, timed, target, clear);
            rp.set_pipeline(pass.pipeline.get());
            rp.set_bind_group(crate::gpu::ONLY_GROUP, &inputs, &[]);
            rp.draw(0..3, 0..1);
        }
    }

    pub(crate) fn transparent_scene_group<'a>(
        &'a self,
        view: &'a RenderView,
    ) -> &'a wgpu::BindGroup {
        let copy = view.post_fx.targets.get::<OpaqueCopy>();
        copy.and_then(|c| c.group.as_ref())
            .unwrap_or(&self.shadow_bind_group)
    }
}
