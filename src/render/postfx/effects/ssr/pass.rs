//! The SSR pass (#1526): two fullscreen passes over the base camera's opaque image,
//! run right after its opaque pass (`render::draw::camera`'s `ssr` stage), in
//! `postfx/ssr.wgsl`. It copies the image into the first level of its colour pyramid
//! ([`pyramid`](super::pyramid), #1736) and builds the rest. The trace (`fs_trace`)
//! then marches each smooth enough pixel's ray and writes its hit into a target of
//! its own, with where it traced and how wide its cone spreads it; their pyramids
//! are built; and the resolve (`fs_resolve`, #2028) averages each pixel's hits over
//! that spread and draws them over the image, so a rough surface's reflected
//! silhouette fades across its blur instead of stepping at a pixel's hit or miss.
//! The translucents, the overlay cameras and every post effect see the reflected
//! scene, HDRP's order.
//!
//! The pipelines are the renderer's ([`SsrPasses`], built on first use and warmed
//! with the rest); the pyramids and the params uniform are each view's
//! ([`SsrTargets`]), held only while it runs SSR. The GPU timer's `ssr` pass spans
//! the trace, the trace's pyramids and the resolve; the colour pyramid is untimed.

use glam::{Mat4, Vec3};

use super::pyramid::{Pyramid, PyramidPass};
use super::record::{begin, opaque_target, pipeline_layout, split};
use super::surface;
use crate::core::settings::quality::SsrQuality;
use crate::render::gpu::bind_entry as entry;
use crate::render::gpu::fullscreen::Fullscreen;
use crate::render::gpu::layout_entries::{sampler_entry, texture_entry, uniform_entry};
use crate::render::gpu::pipelines::LazyPipeline;
use crate::render::gpu::shaders::ShaderRegistry;
use crate::render::postfx::{PostFx, HDR_FORMAT};
use crate::render::timing::{GpuPass, GpuTimer};

/// CPU mirror of `SsrParams` in `postfx/ssr.wgsl`.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct SsrParams {
    inv_view_proj: [f32; 16],
    view_proj: [f32; 16],
    camera_pos: [f32; 4],
    /// x: step count, y: step length (world units).
    march: [f32; 4],
    /// x: the *Minimum Smoothness* (#1736), y: the width it fades in over
    /// ([`super::fade`]).
    surface: [f32; 4],
}

impl SsrParams {
    /// The base camera's frame, marching as `m` ([`super::march`]) says, reflecting
    /// surfaces down to `min_smoothness`.
    pub(crate) fn new(view_proj: Mat4, eye: Vec3, m: SsrQuality, min_smoothness: f32) -> Self {
        Self {
            inv_view_proj: view_proj.inverse().to_cols_array(),
            view_proj: view_proj.to_cols_array(),
            camera_pos: eye.extend(0.0).to_array(),
            march: [m.steps as f32, m.step_length, 0.0, 0.0],
            surface: {
                let [min, width] = super::fade(min_smoothness);
                [min, width, 0.0, 0.0]
            },
        }
    }
}

/// The format of the trace's `spread` target (`postfx/ssr.wgsl`'s `Traced`): whether
/// a pixel traced, and its hit's radius times its weight.
const SPREAD_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rg16Float;

/// The SSR pipelines and their pyramids', shared by every view.
pub(crate) struct SsrPasses {
    layout: wgpu::BindGroupLayout,
    /// The resolve's second group: the trace's two pyramids.
    traced: wgpu::BindGroupLayout,
    trace: LazyPipeline,
    resolve: LazyPipeline,
    pyramid: PyramidPass,
    spread: PyramidPass,
}

impl SsrPasses {
    pub(crate) fn new(device: &wgpu::Device, registry: &mut ShaderRegistry) -> Self {
        let shader = registry.load(device, "postfx/ssr.wgsl", "SSR Shader");
        let d2 = wgpu::TextureViewDimension::D2;
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("SSR Layout"),
            entries: &[
                uniform_entry(0, wgpu::ShaderStages::FRAGMENT),
                texture_entry(1, d2),
                sampler_entry(2),
                texture_entry(3, d2),
                texture_entry(4, d2),
                texture_entry(5, d2),
            ],
        });
        let traced = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("SSR Resolve Layout"),
            entries: &[texture_entry(0, d2), texture_entry(1, d2)],
        });
        let trace_layout = pipeline_layout(device, &[&layout]);
        let resolve_layout = pipeline_layout(device, &[&layout, &traced]);
        let shader2 = shader.clone();
        let trace = LazyPipeline::new(device, move |device| {
            let targets = [HDR_FORMAT, SPREAD_FORMAT].map(|f| Some(opaque_target(f)));
            Fullscreen {
                label: "SSR Trace Pipeline",
                shader: &shader,
                layout: &trace_layout,
                vs: "vs_fullscreen",
                fs: "fs_trace",
            }
            .colour(device, &targets)
        });
        let resolve = LazyPipeline::new(device, move |device| {
            Fullscreen {
                label: "SSR Resolve Pipeline",
                shader: &shader2,
                layout: &resolve_layout,
                vs: "vs_fullscreen",
                fs: "fs_resolve",
            }
            .colour(device, &[Some(opaque_target(HDR_FORMAT))])
        });
        Self {
            layout,
            traced,
            trace,
            resolve,
            pyramid: PyramidPass::new(device, registry),
            spread: PyramidPass::of(device, registry, SPREAD_FORMAT),
        }
    }

    /// The pipelines, for warming (`Renderer::lazy_pipelines`).
    pub(crate) fn lazy_pipelines(&self) -> [&LazyPipeline; 4] {
        [
            &self.trace,
            &self.resolve,
            self.pyramid.lazy_pipeline(),
            self.spread.lazy_pipeline(),
        ]
    }
}

/// One view's SSR state: the pyramid of the opaque image the march reads, the
/// trace's two pyramids the resolve reads, and the params uniform, kept while the
/// view's size holds.
pub(crate) struct SsrTargets {
    size: (u32, u32),
    pyramid: Pyramid,
    hits: Pyramid,
    spread: Pyramid,
    params: wgpu::Buffer,
}

impl SsrTargets {
    fn new(device: &wgpu::Device, passes: &SsrPasses, size: (u32, u32)) -> Self {
        Self {
            size,
            pyramid: Pyramid::new(device, &passes.pyramid, size),
            hits: Pyramid::new(device, &passes.pyramid, size),
            spread: Pyramid::new(device, &passes.spread, size),
            params: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("SSR Params"),
                size: std::mem::size_of::<SsrParams>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
        }
    }
}

impl PostFx {
    /// Reflect this frame's opaque image with `params`, or drop the SSR state when
    /// `params` is `None` or the view wrote no velocity or surface targets.
    pub(crate) fn run_ssr(
        &mut self,
        (device, queue): (&wgpu::Device, &wgpu::Queue),
        passes: &SsrPasses,
        params: Option<SsrParams>,
        timer: &GpuTimer,
    ) {
        let inputs = self.motion.target_view().is_some() && surface::target(self).is_some();
        let Some(params) = params.filter(|_| inputs) else {
            self.targets.remove::<SsrTargets>();
            return;
        };
        let size = self.full_size;
        let fitted = self
            .targets
            .get_or(|| SsrTargets::new(device, passes, size));
        if fitted.size != size {
            *fitted = SsrTargets::new(device, passes, size);
        }
        let t = self.targets.get::<SsrTargets>().expect("fitted above");
        let velocity = self.motion.target_view().expect("checked above");
        let [weight, applied] = surface::views(self).expect("checked above");
        queue.write_buffer(&t.params, 0, bytemuck::bytes_of(&params));
        let view = |v| wgpu::BindingResource::TextureView(v);
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("SSR Bind Group"),
            layout: &passes.layout,
            entries: &[
                entry(0, t.params.as_entire_binding()),
                entry(1, view(&t.pyramid.view)),
                entry(2, wgpu::BindingResource::Sampler(&passes.pyramid.trilinear)),
                entry(3, view(velocity)),
                entry(4, view(weight)),
                entry(5, view(applied)),
            ],
        });
        let traced = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("SSR Resolve Bind Group"),
            layout: &passes.traced,
            entries: &[entry(0, view(&t.hits.view)), entry(1, view(&t.spread.view))],
        });

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("SSR Encoder"),
        });
        let extent = self.scene_hdr.texture.size();
        let (from, to) = (&self.scene_hdr.texture, &t.pyramid.texture);
        encoder.copy_texture_to_texture(from.as_image_copy(), to.as_image_copy(), extent);
        t.pyramid.build(&mut encoder, &passes.pyramid);
        // The timer's `ssr` pair opens on the trace and closes on the resolve, so it
        // counts the trace's pyramids too.
        let timed = timer.writes(GpuPass::Ssr);
        let (open, close) = split(timed);
        {
            // Cleared: a pixel that takes no SSR is discarded, so nothing traced.
            let clear = wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT);
            let targets = [(&t.hits.top, clear), (&t.spread.top, clear)];
            let mut pass = begin(&mut encoder, "SSR Trace", &targets, open);
            pass.set_pipeline(passes.trace.get());
            pass.set_bind_group(0, &group, &[]);
            pass.draw(0..3, 0..1);
        }
        t.hits.build(&mut encoder, &passes.pyramid);
        t.spread.build(&mut encoder, &passes.spread);
        {
            // Loaded: a pixel that gets no SSR is discarded, left as it was.
            let targets = [(&self.scene_hdr.view, wgpu::LoadOp::Load)];
            let mut pass = begin(&mut encoder, "SSR Resolve", &targets, close);
            pass.set_pipeline(passes.resolve.get());
            pass.set_bind_group(0, &group, &[]);
            pass.set_bind_group(1, &traced, &[]);
            pass.draw(0..3, 0..1);
        }
        queue.submit(std::iter::once(encoder.finish()));
    }
}

#[cfg(test)]
impl PostFx {
    /// Whether the view holds SSR's state (it ran this frame).
    pub(crate) fn holds_ssr(&self) -> bool {
        self.targets.get::<SsrTargets>().is_some()
    }

    /// The size and level count of the pyramid SSR read this frame, if it ran.
    pub(crate) fn ssr_pyramid(&self) -> Option<((u32, u32), u32)> {
        let t = self.targets.get::<SsrTargets>()?;
        Some((t.size, t.pyramid.levels()))
    }
}
