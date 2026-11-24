//! *Mixed* for the translucents, as HDRP's transparent depth prepass: the nearest one's
//! surface at the rays' size and rays from it, bound as `t_ssr_rays`.

use super::super::record::{begin, opaque_target, pipeline_layout};
use super::super::targets::SsrTargets;
use super::targets::{ray_size, RayTargets};
use crate::draw::batch::DrawBatch;
use crate::gpu::bind_entry as entry;
use crate::gpu::fullscreen::Fullscreen;
use crate::gpu::groups;
use crate::gpu::layout_entries::texture_entry;
use crate::gpu::ops::{CountedDevice as _, CountedEncoder as _};
use crate::gpu::pipelines::surface::SolidPass;
use crate::gpu::pipelines::LazyPipeline;
use crate::gpu::shaders::ShaderRegistry;
use crate::gpu::DEPTH_FORMAT;
use crate::postfx::HDR_FORMAT;
use crate::scene::MaterialRecords;
use crate::timing::GpuPass;
use crate::{MaterialData, RenderView, Renderer};

const INPUTS: u32 = 0;
const SURFACE: u32 = 1;
const SHADE: u32 = 2;
const LAYERS: u32 = 3;

pub(crate) struct TransparentRayPass {
    surface: wgpu::BindGroupLayout,
    ray: LazyPipeline,
    bound: LazyPipeline,
}

impl TransparentRayPass {
    pub(super) fn new(
        device: &wgpu::Device,
        registry: &mut ShaderRegistry,
        (inputs, shade): (&wgpu::BindGroupLayout, &wgpu::BindGroupLayout),
    ) -> Self {
        let surface = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("SSR Transparent Surface Layout"),
            entries: &[texture_entry(0, wgpu::TextureViewDimension::D2)],
        });
        let tree = crate::rays::layout(device);
        let layout = pipeline_layout(device, &[inputs, &surface, shade, &tree]);
        let ray = fullscreen(
            device,
            registry,
            ("postfx/ssr_transparent.wgsl", "fs_ray"),
            layout,
        );
        let layout = pipeline_layout(device, &[&surface]);
        let shader = ("postfx/ssr_transparent_bound.wgsl", "fs_bound");
        let bound = fullscreen(device, registry, shader, layout);
        Self {
            surface,
            ray,
            bound,
        }
    }

    pub(super) fn lazy_pipelines(&self) -> [&LazyPipeline; 2] {
        [&self.ray, &self.bound]
    }
}

fn fullscreen(
    device: &wgpu::Device,
    registry: &mut ShaderRegistry,
    (path, entry): (&str, &'static str),
    layout: wgpu::PipelineLayout,
) -> LazyPipeline {
    let shader = registry.load(device, path, path);
    LazyPipeline::new(device, move |device| {
        Fullscreen {
            label: entry,
            shader: &shader,
            layout: &layout,
            vs: "vs_fullscreen",
            fs: entry,
        }
        .colour(device, &[Some(opaque_target(HDR_FORMAT))])
    })
}

/// Layer 0 the surface, 1 its ray, 2 the ray bounded.
#[derive(Clone)]
pub(crate) struct TransparentRays {
    size: (u32, u32),
    surface: wgpu::TextureView,
    ray: wgpu::TextureView,
    bounded: wgpu::TextureView,
    pub(crate) layers: wgpu::TextureView,
    depth: wgpu::TextureView,
    groups: [wgpu::BindGroup; 2],
}

impl TransparentRays {
    fn new(device: &wgpu::Device, layout: &wgpu::BindGroupLayout, size: (u32, u32)) -> Self {
        use wgpu::TextureUsages as U;
        let extent = |layers| wgpu::Extent3d {
            width: size.0.max(1),
            height: size.1.max(1),
            depth_or_array_layers: layers,
        };
        let texture = |label, layers, format| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: extent(layers),
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: U::RENDER_ATTACHMENT | U::TEXTURE_BINDING,
                view_formats: &[],
            })
        };
        let both = texture("SSR Transparent Rays", LAYERS, HDR_FORMAT);
        let layer = |base_array_layer| {
            both.create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(wgpu::TextureViewDimension::D2),
                base_array_layer,
                array_layer_count: Some(1),
                ..Default::default()
            })
        };
        let (surface, ray, bounded) = (layer(0), layer(1), layer(2));
        let group = |label, view| {
            device.counted_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(label),
                layout,
                entries: &[entry(0, wgpu::BindingResource::TextureView(view))],
            })
        };
        let groups = [
            group("SSR Transparent Surface Bind Group", &surface),
            group("SSR Transparent Rays Bind Group", &ray),
        ];
        let depth = texture("SSR Transparent Depth", 1, DEPTH_FORMAT);
        Self {
            size,
            surface,
            ray,
            bounded,
            layers: both.create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(wgpu::TextureViewDimension::D2Array),
                ..Default::default()
            }),
            depth: depth.create_view(&Default::default()),
            groups,
        }
    }

    pub(crate) fn texels(&self) -> u64 {
        u64::from(self.size.0) * u64::from(self.size.1)
    }
}

/// A map, a detail or a decal may smooth it anywhere.
fn smoothest(m: &MaterialData) -> f32 {
    let mapped = m.use_roughness_map != 0 || m.detail.use_detail_map != 0;
    match mapped || m.receive_decals != 0 {
        true => 1.0,
        false => 1.0 - m.roughness.max(0.04),
    }
}

/// `receive_ssr` already leaves out one rougher than the march's floor.
pub(crate) fn tracing<'a>(
    translucents: &'a [DrawBatch],
    records: &MaterialRecords,
    threshold: f32,
) -> Vec<&'a DrawBatch> {
    let reaches = |b: &&DrawBatch| smoothest(records.table().get(b.key.record)) >= threshold;
    let receives = translucents.iter().filter(|b| b.key.traits.receive_ssr);
    receives.filter(reaches).collect()
}

impl Renderer {
    /// Base camera only; returns the texels traced.
    pub(crate) fn trace_transparent_rays(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        view: &mut RenderView,
        translucents: &[DrawBatch],
        rays: Option<[f32; 4]>,
    ) -> u64 {
        let Some((divisor, [inputs, shade, tree], batches)) =
            self.transparent_ray_inputs(view, translucents, rays)
        else {
            view.post_fx.targets.remove::<TransparentRays>();
            return 0;
        };
        let t = self.fit_transparent_rays(view, divisor);
        self.draw_transparent_surface(encoder, view, &t, &batches);
        let passes = &self.post_passes.ssr.rays.transparent;
        let clear = wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT);
        {
            let timed = self.gpu_timer.writes(GpuPass::SsrTransparentRays);
            let mut pass = begin(encoder, "SSR Transparent Rays", &[(&t.ray, clear)], timed);
            pass.set_pipeline(passes.ray.get());
            pass.set_bind_group(INPUTS, &inputs, &[]);
            pass.set_bind_group(SURFACE, &t.groups[0], &[]);
            pass.set_bind_group(SHADE, &shade, &[]);
            pass.set_bind_group(crate::rays::GROUP, &tree, &[]);
            pass.draw(0..3, 0..1);
        }
        {
            let timed = self.gpu_timer.writes(GpuPass::SsrTransparentRays);
            let mut pass = begin(
                encoder,
                "SSR Transparent Bound",
                &[(&t.bounded, clear)],
                timed,
            );
            pass.set_pipeline(passes.bound.get());
            pass.set_bind_group(0, &t.groups[1], &[]);
            pass.draw(0..3, 0..1);
        }
        self.bind_ssr_rays(view, &t.layers);
        t.texels()
    }

    /// `None` unless the opaque rays traced this frame and a translucent can take one.
    fn transparent_ray_inputs(
        &mut self,
        view: &RenderView,
        translucents: &[DrawBatch],
        rays: Option<[f32; 4]>,
    ) -> Option<(u32, [wgpu::BindGroup; 3], Vec<DrawBatch>)> {
        let [threshold, _, divisor, _] = rays?;
        view.post_fx.targets.get::<RayTargets>()?;
        let inputs = view
            .post_fx
            .targets
            .get::<SsrTargets>()?
            .frame_group()?
            .clone();
        let tracing = tracing(translucents, &self.retained.records, threshold);
        let batches: Vec<DrawBatch> = tracing.into_iter().cloned().collect();
        if batches.is_empty() {
            return None;
        }
        let (tree, shade) = self.ssr_ray_groups()?;
        Some((divisor as u32, [inputs, shade, tree], batches))
    }

    fn fit_transparent_rays(&self, view: &mut RenderView, divisor: u32) -> TransparentRays {
        let size = ray_size(view.post_fx.full_size, divisor);
        let layout = &self.post_passes.ssr.rays.transparent.surface;
        let make = || TransparentRays::new(&self.device, layout, size);
        let t = view.post_fx.targets.get_or(make);
        if t.size != size {
            *t = make();
        }
        t.clone()
    }

    /// Hidden by the opaque copy's eye distance: the view's depth is not at the rays' size.
    fn draw_transparent_surface(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        view: &RenderView,
        t: &TransparentRays,
        batches: &[DrawBatch],
    ) {
        let colour = [Some(wgpu::RenderPassColorAttachment {
            view: &t.surface,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                store: wgpu::StoreOp::Store,
            },
        })];
        let mut pass = encoder.counted_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("SSR Transparent Surface"),
            color_attachments: &colour,
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &t.depth,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Discard,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: self.gpu_timer.writes(GpuPass::SsrTransparentRays),
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_bind_group(groups::GLOBAL, &self.global_bind_group, &[]);
        pass.set_bind_group(groups::SCENE, self.transparent_scene_group(view), &[]);
        let pipelines = SolidPass::SsrSurface(&self.transparent_pipelines);
        self.draw_batches(&mut pass, batches, pipelines);
    }
}
