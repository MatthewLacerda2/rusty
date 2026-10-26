//! src/render/postfx/depth_of_field/ — depth of field (#1445), Unity URP's Gaussian
//! and Bokeh modes, in `postfx/depth_of_field.wgsl`.
//!
//! Runs first in the chain, on the HDR scene, before motion blur (Unity's order):
//! the **CoC** pass turns each pixel's distance into a signed circle of confusion;
//! the **prefilter** brings colour and CoC to the working resolution (half on Medium,
//! full on High, off on Low); the **blur** is a separable Gaussian (two passes) or a
//! Bokeh disc gather with near and far fields kept apart plus a tent filter; the
//! **composite** blends the sharp scene towards the blur by each pixel's CoC. Its
//! output is what motion blur, bloom and the composite read
//! ([`super::PostFx::scene_color`]).
//!
//! **The distance is each pixel's own camera's**, from the velocity target (#1435,
//! #1436), never the depth buffer, which a `DepthOnly` overlay has cleared. A pixel
//! an overlay camera drew (a viewmodel's gun) has no CoC and is composited sharp:
//! the world's focus never reaches it. So the velocity target is written whenever a
//! view runs depth of field, not only motion blur (`motion::wanted`).
//!
//! The pipelines are the renderer's ([`DofPasses`], built on first use and warmed);
//! the targets are each view's ([`DofTargets`]), held only while it runs.

mod params;

pub(crate) use params::DofParams;

use crate::render::gpu::fullscreen::Fullscreen;
use crate::render::gpu::layout_entries::{texture_entry, uniform_entry};
use crate::render::gpu::pipelines::LazyPipeline;
use crate::render::gpu::shaders::ShaderRegistry;
use crate::render::postfx::{PfxTarget, PostFx, HDR_FORMAT};
use crate::render::timing::{GpuPass, GpuTimer};

/// The full-resolution CoC's format: a signed radius in pixels.
const COC_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R16Float;

/// The depth-of-field pipelines, shared by every view.
pub(crate) struct DofPasses {
    layout: wgpu::BindGroupLayout,
    coc: LazyPipeline,
    prefilter: LazyPipeline,
    gauss_h: LazyPipeline,
    gauss_v: LazyPipeline,
    bokeh: LazyPipeline,
    tent: LazyPipeline,
    composite: LazyPipeline,
}

impl DofPasses {
    pub(crate) fn new(device: &wgpu::Device, registry: &mut ShaderRegistry) -> Self {
        let shader = registry.load(device, "postfx/depth_of_field.wgsl", "DoF Shader");
        let d2 = wgpu::TextureViewDimension::D2;
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("DoF Layout"),
            entries: &[
                uniform_entry(0, wgpu::ShaderStages::FRAGMENT),
                texture_entry(1, d2),
                texture_entry(2, d2),
                texture_entry(3, d2),
                texture_entry(4, d2),
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("DoF Pipeline Layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pass = |fs: &'static str, format| {
            let (shader, layout) = (shader.clone(), pipeline_layout.clone());
            LazyPipeline::new(device, move |device| {
                let target = wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                };
                let label = format!("DoF {fs}");
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
            coc: pass("fs_coc", COC_FORMAT),
            prefilter: pass("fs_prefilter", HDR_FORMAT),
            gauss_h: pass("fs_gauss_h", HDR_FORMAT),
            gauss_v: pass("fs_gauss_v", HDR_FORMAT),
            bokeh: pass("fs_bokeh", HDR_FORMAT),
            tent: pass("fs_tent", HDR_FORMAT),
            composite: pass("fs_composite", HDR_FORMAT),
            layout,
        }
    }

    /// The pipelines, for warming (`Renderer::lazy_pipelines`).
    pub(crate) fn lazy_pipelines(&self) -> [&LazyPipeline; 7] {
        [
            &self.coc,
            &self.prefilter,
            &self.gauss_h,
            &self.gauss_v,
            &self.bokeh,
            &self.tent,
            &self.composite,
        ]
    }
}

/// One view's depth-of-field targets, kept while its size and working divisor hold.
pub(crate) struct DofTargets {
    /// The full size and divisor these were made for.
    key: ((u32, u32), u32),
    /// The full-resolution CoC.
    coc: PfxTarget,
    /// The working-resolution ping-pong pair; the blur ends in `a`.
    a: PfxTarget,
    b: PfxTarget,
    /// The finished HDR scene motion blur, bloom and the composite read.
    pub(crate) focused: PfxTarget,
    params: wgpu::Buffer,
}

impl DofTargets {
    fn new(device: &wgpu::Device, (w, h): (u32, u32), divisor: u32) -> Self {
        let (ww, wh) = ((w / divisor).max(1), (h / divisor).max(1));
        let target = |w, h, label, format| PostFx::target(device, w, h, label, format);
        Self {
            key: ((w, h), divisor),
            coc: target(w, h, "DoF CoC", COC_FORMAT),
            a: target(ww, wh, "DoF A", HDR_FORMAT),
            b: target(ww, wh, "DoF B", HDR_FORMAT),
            focused: target(w, h, "DoF Scene", HDR_FORMAT),
            params: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("DoF Params"),
                size: std::mem::size_of::<DofParams>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
        }
    }
}

impl PostFx {
    /// Blur this frame's scene by depth with `params`, or drop the targets when
    /// `params` is `None` or the view wrote no velocity target this frame.
    /// Afterwards [`PostFx::focused_color`] is the image motion blur reads.
    pub(crate) fn run_depth_of_field(
        &mut self,
        (device, queue): (&wgpu::Device, &wgpu::Queue),
        passes: &DofPasses,
        params: Option<DofParams>,
        timer: &GpuTimer,
    ) {
        let Some((params, velocity)) = params.zip(self.motion.target_view()) else {
            self.dof = None;
            return;
        };
        let key = (self.full_size, params.divisor());
        if self.dof.as_ref().is_none_or(|t| t.key != key) {
            self.dof = Some(DofTargets::new(device, key.0, key.1));
        }
        let t = self.dof.as_ref().expect("fitted above");
        queue.write_buffer(&t.params, 0, bytemuck::bytes_of(&params));
        // (pass input, CoC, blur): each pass binds no texture it writes.
        let group = |input: &PfxTarget, coc: &wgpu::TextureView, blur: &PfxTarget| {
            let view = |v| wgpu::BindingResource::TextureView(v);
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("DoF Bind Group"),
                layout: &passes.layout,
                entries: &[
                    entry(0, t.params.as_entire_binding()),
                    entry(1, view(&input.view)),
                    entry(2, view(velocity)),
                    entry(3, view(coc)),
                    entry(4, view(&blur.view)),
                ],
            })
        };
        let (scene, coc) = (&self.scene_hdr, &t.coc.view);
        let mut steps = vec![
            (&passes.coc, group(scene, velocity, &t.a), &t.coc),
            (&passes.prefilter, group(scene, coc, &t.b), &t.a),
        ];
        let blur = if params.shape[0] < 1.5 {
            [(&passes.gauss_h, &t.a, &t.b), (&passes.gauss_v, &t.b, &t.a)]
        } else {
            [(&passes.bokeh, &t.a, &t.b), (&passes.tent, &t.b, &t.a)]
        };
        for (pipeline, input, target) in blur {
            steps.push((pipeline, group(input, coc, input), target));
        }
        steps.push((&passes.composite, group(scene, coc, &t.a), &t.focused));

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("DoF Encoder"),
        });
        for (pipeline, bind_group, target) in &steps {
            let timed = (pipeline.get(), timer.writes(GpuPass::DepthOfField));
            Self::fullscreen(&mut encoder, timed, &[bind_group], &target.view);
        }
        queue.submit(std::iter::once(encoder.finish()));
    }
}

fn entry(binding: u32, resource: wgpu::BindingResource<'_>) -> wgpu::BindGroupEntry<'_> {
    wgpu::BindGroupEntry { binding, resource }
}

#[cfg(test)]
mod gpu_fixture;

#[cfg(test)]
mod gpu_tests;

#[cfg(test)]
mod overlay_gpu_tests;
