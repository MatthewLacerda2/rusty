//! View depths come from the velocity target, never the depth buffer, which a `DepthOnly`
//! overlay has cleared.

mod effect;
mod params;

use crate::gpu::ops::{CountedDevice as _, CountedQueue as _};
pub(crate) use effect::{dof_enabled, EFFECT};
pub(crate) use params::DofParams;

use crate::gpu::bind_entry as entry;
use crate::gpu::fullscreen::Fullscreen;
use crate::gpu::layout_entries::{texture_entry, uniform_entry};
use crate::gpu::pipelines::LazyPipeline;
use crate::gpu::shaders::ShaderRegistry;
use crate::postfx::{PfxTarget, PostFx, HDR_FORMAT};
use crate::timing::{GpuPass, GpuTimer};

/// A signed radius in pixels; the Bokeh tiles' near reach uses it too.
const COC_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R16Float;

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

pub(crate) struct DofTargets {
    key: ((u32, u32), u32, u32),
    coc: PfxTarget,
    work: PfxTarget,
    a: PfxTarget,
    b: PfxTarget,
    tiles: Option<(PfxTarget, PfxTarget)>,
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

type Step<'a> = (&'a LazyPipeline, [&'a wgpu::TextureView; 4], &'a PfxTarget);

impl DofTargets {
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
    pub(crate) fn run_depth_of_field(
        &mut self,
        (device, queue): (&wgpu::Device, &wgpu::Queue),
        encoder: &mut wgpu::CommandEncoder,
        passes: &DofPasses,
        params: Option<DofParams>,
        timer: &GpuTimer,
    ) {
        let Some((params, velocity)) = params.zip(self.motion.target_view()) else {
            self.targets.remove::<DofTargets>();
            return;
        };
        let key = (self.full_size, params.divisor(), params.tile());
        let fitted = self.targets.get_or(|| DofTargets::new(device, key));
        if fitted.key != key {
            *fitted = DofTargets::new(device, key);
        }
        let t = self.targets.get::<DofTargets>().expect("fitted above");
        queue.counted_write_buffer(&t.params, 0, bytemuck::bytes_of(&params));
        let group = |[input, coc, blur, work]: [&wgpu::TextureView; 4]| {
            let view = |v| wgpu::BindingResource::TextureView(v);
            device.counted_bind_group(&wgpu::BindGroupDescriptor {
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

        for (pipeline, bind_group, target) in &steps {
            let timed = (pipeline.get(), timer.writes(GpuPass::DepthOfField));
            Self::fullscreen(encoder, timed, &[bind_group], &target.view);
        }
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

#[cfg(test)]
impl PostFx {
    pub(crate) fn holds_dof(&self) -> bool {
        self.targets.get::<DofTargets>().is_some()
    }
}
