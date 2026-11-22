//! Depths come from the velocity target each camera writes, never the depth buffer, which a
//! `DepthOnly` overlay has cleared.

pub(crate) mod effect;
mod params;

use crate::gpu::ops::{CountedDevice as _, CountedQueue as _};
pub(crate) use effect::{motion_blur_enabled, EFFECT};
pub(crate) use params::BlurParams;

use super::Targets;
use crate::gpu::bind_entry as entry;
use crate::gpu::fullscreen::Fullscreen;
use crate::gpu::layout_entries::{texture_entry, uniform_entry};
use crate::gpu::pipelines::LazyPipeline;
use crate::gpu::shaders::ShaderRegistry;
use crate::postfx::{PfxTarget, PostFx, HDR_FORMAT};
use crate::timing::{GpuPass, GpuTimer};

/// A streak in pixels.
const TILE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rg16Float;

pub(crate) struct MotionBlurPasses {
    layout: wgpu::BindGroupLayout,
    tile_max_x: LazyPipeline,
    tile_max_y: LazyPipeline,
    neighbor_max: LazyPipeline,
    gather: LazyPipeline,
}

impl MotionBlurPasses {
    pub(crate) fn new(device: &wgpu::Device, registry: &mut ShaderRegistry) -> Self {
        let shader = registry.load(device, "postfx/motion_blur.wgsl", "Motion Blur Shader");
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Motion Blur Layout"),
            entries: &[
                uniform_entry(0, wgpu::ShaderStages::FRAGMENT),
                texture_entry(1, wgpu::TextureViewDimension::D2),
                texture_entry(2, wgpu::TextureViewDimension::D2),
                texture_entry(3, wgpu::TextureViewDimension::D2),
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Motion Blur Pipeline Layout"),
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
                let label = format!("Motion Blur {fs}");
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
            tile_max_x: pass("fs_tile_max_x", TILE_FORMAT),
            tile_max_y: pass("fs_tile_max_y", TILE_FORMAT),
            neighbor_max: pass("fs_neighbor_max", TILE_FORMAT),
            gather: pass("fs_gather", HDR_FORMAT),
            layout,
        }
    }

    pub(crate) fn lazy_pipelines(&self) -> [&LazyPipeline; 4] {
        [
            &self.tile_max_x,
            &self.tile_max_y,
            &self.neighbor_max,
            &self.gather,
        ]
    }
}

pub(crate) struct BlurTargets {
    size: (u32, u32),
    pub(crate) blurred: PfxTarget,
    params: wgpu::Buffer,
    tiles: TileTargets,
}

struct TileTargets {
    tile: u32,
    rows: PfxTarget,
    tile_max: PfxTarget,
    neighbor_max: PfxTarget,
}

impl TileTargets {
    fn new(device: &wgpu::Device, (w, h): (u32, u32), tile: u32) -> Self {
        let (tw, th) = (params::tiles(w, tile), params::tiles(h, tile));
        let target = |w, h, label| PostFx::target(device, w, h, label, TILE_FORMAT);
        Self {
            tile,
            rows: target(tw, h, "Motion Blur TileMax Rows"),
            tile_max: target(tw, th, "Motion Blur TileMax"),
            neighbor_max: target(tw, th, "Motion Blur NeighborMax"),
        }
    }
}

impl BlurTargets {
    fn new(device: &wgpu::Device, (w, h): (u32, u32), tile: u32) -> Self {
        Self {
            size: (w, h),
            blurred: PostFx::target(device, w, h, "Motion Blur Scene", HDR_FORMAT),
            params: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Motion Blur Params"),
                size: std::mem::size_of::<BlurParams>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            tiles: TileTargets::new(device, (w, h), tile),
        }
    }

    fn fit(targets: &mut Targets, device: &wgpu::Device, size: (u32, u32), tile: u32) {
        let t = targets.get_or(|| Self::new(device, size, tile));
        if t.size != size {
            *t = Self::new(device, size, tile);
        } else if t.tiles.tile != tile {
            t.tiles = TileTargets::new(device, size, tile);
        }
    }
}

impl PostFx {
    pub(crate) fn run_motion_blur(
        &mut self,
        (device, queue): (&wgpu::Device, &wgpu::Queue),
        encoder: &mut wgpu::CommandEncoder,
        passes: &MotionBlurPasses,
        params: Option<BlurParams>,
        timer: &GpuTimer,
    ) {
        let Some((params, velocity)) = params.zip(self.motion.target_view()) else {
            self.targets.remove::<BlurTargets>();
            return;
        };
        BlurTargets::fit(&mut self.targets, device, self.chain_size, params.tile());
        let targets = self.targets.get::<BlurTargets>().expect("fitted above");
        let tiles = &targets.tiles;
        queue.counted_write_buffer(&targets.params, 0, bytemuck::bytes_of(&params));
        let group = |input: &wgpu::TextureView| {
            device.counted_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Motion Blur Bind Group"),
                layout: &passes.layout,
                entries: &[
                    entry(0, targets.params.as_entire_binding()),
                    entry(1, wgpu::BindingResource::TextureView(self.resolved_color())),
                    entry(2, wgpu::BindingResource::TextureView(velocity)),
                    entry(3, wgpu::BindingResource::TextureView(input)),
                ],
            })
        };
        // Each pass reads the one before at binding 3; the first binds a texture it does not write.
        let steps = [
            (&passes.tile_max_x, &tiles.neighbor_max, &tiles.rows),
            (&passes.tile_max_y, &tiles.rows, &tiles.tile_max),
            (&passes.neighbor_max, &tiles.tile_max, &tiles.neighbor_max),
            (&passes.gather, &tiles.neighbor_max, &targets.blurred),
        ];
        for (pipeline, input, target) in steps {
            let timed = (pipeline.get(), timer.writes(GpuPass::MotionBlur));
            Self::fullscreen(encoder, timed, &[&group(&input.view)], &target.view);
        }
    }
}

#[cfg(test)]
mod gpu_tests;

#[cfg(test)]
pub(crate) mod overlay_gpu_tests;

#[cfg(test)]
pub(crate) mod stack_order_gpu_tests;

#[cfg(test)]
impl PostFx {
    pub(crate) fn holds_blur(&self) -> bool {
        self.targets.get::<BlurTargets>().is_some()
    }
}
