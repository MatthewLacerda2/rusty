//! src/render/postfx/depth_of_field/ — depth of field (#1445), Unity URP's Gaussian
//! and Bokeh modes, in `postfx/depth_of_field.wgsl`.
//!
//! Runs first in the chain, on the HDR scene, before motion blur (Unity's order):
//! the **CoC** pass turns each pixel's view depth into a signed circle of confusion;
//! the **prefilter** brings colour and CoC to the working resolution (half on Medium,
//! full on High, off on Low); the **blur** is a separable Gaussian (two passes) or a
//! Bokeh disc gather with near and far fields kept apart plus a tent filter. The
//! gather spreads its taps over each pixel's own reach (its CoC, or the near blur a
//! tile pass finds around it), so a 2 px blur is as dense as a 27 px one. The
//! **composite** blends the sharp scene towards the blur by each pixel's CoC. Its
//! output is what motion blur, bloom and the composite read
//! ([`super::PostFx::scene_color`]).
//!
//! **The depth is each pixel's own camera's**, along its view axis, from the
//! velocity target (#1435, #1436), never the depth buffer, which a `DepthOnly`
//! overlay has cleared. Linear view depth, not the straight-line distance, so the
//! focus is a plane: a wall at the focus distance is sharp to its corners. A pixel
//! an overlay camera drew (a viewmodel's gun) has no CoC and is composited sharp:
//! the world's focus never reaches it. So the velocity target is written whenever a
//! view runs depth of field, not only motion blur (`motion::wanted`).
//!
//! The pipelines are the renderer's ([`DofPasses`], built on first use and warmed);
//! the targets are each view's ([`DofTargets`]), held only while it runs.

mod effect;
mod params;

pub(crate) use effect::{dof_enabled, EFFECT};
pub(crate) use params::DofParams;

use crate::render::gpu::bind_entry as entry;
use crate::render::gpu::fullscreen::Fullscreen;
use crate::render::gpu::layout_entries::{texture_entry, uniform_entry};
use crate::render::gpu::pipelines::LazyPipeline;
use crate::render::gpu::shaders::ShaderRegistry;
use crate::render::postfx::{PfxTarget, PostFx, HDR_FORMAT};
use crate::render::timing::{GpuPass, GpuTimer};

/// The full-resolution CoC's format: a signed radius in pixels. The Bokeh tiles'
/// near reach uses it too.
const COC_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R16Float;

/// The depth-of-field pipelines, shared by every view.
pub(crate) struct DofPasses {
    layout: wgpu::BindGroupLayout,
    coc: LazyPipeline,
    prefilter: LazyPipeline,
    tile_max: LazyPipeline,
    neighbor_max: LazyPipeline,
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
                texture_entry(5, d2),
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
            tile_max: pass("fs_tile_max", COC_FORMAT),
            neighbor_max: pass("fs_neighbor_max", COC_FORMAT),
            gauss_h: pass("fs_gauss_h", HDR_FORMAT),
            gauss_v: pass("fs_gauss_v", HDR_FORMAT),
            bokeh: pass("fs_bokeh", HDR_FORMAT),
            tent: pass("fs_tent", HDR_FORMAT),
            composite: pass("fs_composite", HDR_FORMAT),
            layout,
        }
    }

    /// The pipelines, for warming (`Renderer::lazy_pipelines`).
    pub(crate) fn lazy_pipelines(&self) -> [&LazyPipeline; 9] {
        [
            &self.coc,
            &self.prefilter,
            &self.tile_max,
            &self.neighbor_max,
            &self.gauss_h,
            &self.gauss_v,
            &self.bokeh,
            &self.tent,
            &self.composite,
        ]
    }
}

/// One view's depth-of-field targets, kept while its size, working divisor and
/// Bokeh tile size hold.
pub(crate) struct DofTargets {
    /// The full size, divisor and tile size (0: Gaussian, no tiles) these were made for.
    key: ((u32, u32), u32, u32),
    /// The full-resolution CoC.
    coc: PfxTarget,
    /// The prefiltered working image, kept for its reduced CoC.
    work: PfxTarget,
    /// The working-resolution ping-pong pair; the blur ends in `a`.
    a: PfxTarget,
    b: PfxTarget,
    /// Bokeh's tiles: each tile's near reach, then its 3×3 neighbourhood's.
    tiles: Option<(PfxTarget, PfxTarget)>,
    /// The finished HDR scene motion blur, bloom and the composite read.
    pub(crate) focused: PfxTarget,
    params: wgpu::Buffer,
}

impl DofTargets {
    fn new(device: &wgpu::Device, key: ((u32, u32), u32, u32)) -> Self {
        let ((w, h), divisor, tile) = key;
        let (ww, wh) = ((w / divisor).max(1), (h / divisor).max(1));
        let target = |w, h, label, format| PostFx::target(device, w, h, label, format);
        let tiles = (tile > 0).then(|| {
            let (tw, th) = (ww.div_ceil(tile), wh.div_ceil(tile));
            let tiles = target(tw, th, "DoF Tiles", COC_FORMAT);
            (tiles, target(tw, th, "DoF Neighbours", COC_FORMAT))
        });
        Self {
            key,
            coc: target(w, h, "DoF CoC", COC_FORMAT),
            work: target(ww, wh, "DoF Work", HDR_FORMAT),
            a: target(ww, wh, "DoF A", HDR_FORMAT),
            b: target(ww, wh, "DoF B", HDR_FORMAT),
            tiles,
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

/// One pass of the chain: its pipeline, the views it binds (input, CoC, blur,
/// work) and the target it writes.
type Step<'a> = (&'a LazyPipeline, [&'a wgpu::TextureView; 4], &'a PfxTarget);

impl DofTargets {
    /// The passes in order. Each binds no texture it writes, and every pass after
    /// the prefilter reads its reduced CoC from `work`; the CoC pass reads the
    /// view depths from the velocity target and the prefilter, which writes `work`,
    /// binds the full CoC in its place.
    fn plan<'a>(
        &'a self,
        passes: &'a DofPasses,
        scene: &'a wgpu::TextureView,
        velocity: &'a wgpu::TextureView,
    ) -> Vec<Step<'a>> {
        let (coc, work, a, b) = (&self.coc.view, &self.work.view, &self.a.view, &self.b.view);
        let mut steps: Vec<Step<'a>> = vec![
            (&passes.coc, [scene, velocity, a, b], &self.coc),
            (&passes.prefilter, [scene, coc, b, coc], &self.work),
        ];
        match &self.tiles {
            None => {
                steps.push((&passes.gauss_h, [work, coc, a, work], &self.b));
                steps.push((&passes.gauss_v, [b, coc, b, work], &self.a));
            }
            Some((tiles, near)) => {
                steps.push((&passes.tile_max, [work, coc, a, work], tiles));
                steps.push((&passes.neighbor_max, [&tiles.view, coc, a, work], near));
                steps.push((&passes.bokeh, [work, coc, &near.view, work], &self.b));
                steps.push((&passes.tent, [b, coc, b, work], &self.a));
            }
        }
        steps.push((&passes.composite, [scene, coc, a, work], &self.focused));
        steps
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
        let key = (self.full_size, params.divisor(), params.tile());
        if self.dof.as_ref().is_none_or(|t| t.key != key) {
            self.dof = Some(DofTargets::new(device, key));
        }
        let t = self.dof.as_ref().expect("fitted above");
        queue.write_buffer(&t.params, 0, bytemuck::bytes_of(&params));
        let group = |[input, coc, blur, work]: [&wgpu::TextureView; 4]| {
            let view = |v| wgpu::BindingResource::TextureView(v);
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("DoF Bind Group"),
                layout: &passes.layout,
                entries: &[
                    entry(0, t.params.as_entire_binding()),
                    entry(1, view(input)),
                    entry(2, view(velocity)),
                    entry(3, view(coc)),
                    entry(4, view(blur)),
                    entry(5, view(work)),
                ],
            })
        };
        let steps: Vec<_> = t
            .plan(passes, &self.scene_hdr.view, velocity)
            .into_iter()
            .map(|(pipeline, views, target)| (pipeline, group(views), target))
            .collect();

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

#[cfg(test)]
mod gpu_fixture;

#[cfg(test)]
mod gpu_tests;

#[cfg(test)]
mod overlay_gpu_tests;

#[cfg(test)]
mod lens_gpu_tests;

#[cfg(test)]
mod tile_gpu_tests;
