//! The SSR pass (#1526): one fullscreen pass over the base camera's opaque image, run
//! right after its opaque pass (`render::draw::camera`'s `ssr` stage), in
//! `postfx/ssr.wgsl`. It copies the image into the first level of its colour pyramid
//! ([`pyramid`](super::pyramid), #1736) and builds the rest, then draws over the image
//! where a smooth enough surface's march hits, so the translucents, the overlay
//! cameras and every post effect see the reflected scene, HDRP's order.
//!
//! The pipelines are the renderer's ([`SsrPasses`], built on first use and warmed
//! with the rest); the pyramid and the params uniform are each view's
//! ([`SsrTargets`]), held only while it runs SSR.

use glam::{Mat4, Vec3};

use super::pyramid::{Pyramid, PyramidPass};
use super::surface;
use crate::core::settings::quality::SsrQuality;
use crate::render::gpu::bind_entry as entry;
use crate::render::gpu::fullscreen::{color_pass, Fullscreen};
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

/// The SSR pipeline and its pyramid's, shared by every view.
pub(crate) struct SsrPasses {
    layout: wgpu::BindGroupLayout,
    pipeline: LazyPipeline,
    pyramid: PyramidPass,
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
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("SSR Pipeline Layout"),
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
                label: "SSR Pipeline",
                shader: &shader,
                layout: &pipeline_layout,
                vs: "vs_fullscreen",
                fs: "fs_ssr",
            }
            .colour(device, &[Some(target)])
        });
        let pyramid = PyramidPass::new(device, registry);
        Self {
            layout,
            pipeline,
            pyramid,
        }
    }

    /// The pipelines, for warming (`Renderer::lazy_pipelines`).
    pub(crate) fn lazy_pipelines(&self) -> [&LazyPipeline; 2] {
        [&self.pipeline, self.pyramid.lazy_pipeline()]
    }
}

/// One view's SSR state: the pyramid of the opaque image the march reads, and the
/// params uniform, kept while the view's size holds.
pub(crate) struct SsrTargets {
    size: (u32, u32),
    pyramid: Pyramid,
    params: wgpu::Buffer,
}

impl SsrTargets {
    fn new(device: &wgpu::Device, passes: &SsrPasses, size: (u32, u32)) -> Self {
        Self {
            size,
            pyramid: Pyramid::new(device, &passes.pyramid, size),
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

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("SSR Encoder"),
        });
        let extent = self.scene_hdr.texture.size();
        let (from, to) = (&self.scene_hdr.texture, &t.pyramid.texture);
        encoder.copy_texture_to_texture(from.as_image_copy(), to.as_image_copy(), extent);
        t.pyramid.build(&mut encoder, &passes.pyramid);
        {
            // Loaded: a pixel that gets no SSR is discarded, left as it was.
            let timed = ("SSR Pass", timer, GpuPass::Ssr);
            let load = wgpu::LoadOp::Load;
            let mut pass = color_pass(&mut encoder, timed, &self.scene_hdr.view, load);
            pass.set_pipeline(passes.pipeline.get());
            pass.set_bind_group(0, &group, &[]);
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
