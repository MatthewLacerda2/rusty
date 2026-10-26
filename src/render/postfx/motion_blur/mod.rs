//! src/render/postfx/motion_blur/ — per-object motion blur (#1436): the
//! reconstruction filter over a view's velocity target (#1435), in
//! `postfx/motion_blur.wgsl`.
//!
//! Four fullscreen passes, before bloom and the composite: **TileMax** keeps each
//! tile's longest streak, in two steps (each tile row's, then the tile's) so a big
//! tile is two short loops; **NeighborMax** spreads each tile's to its 3×3
//! neighbourhood (so the blur reaches past a moving silhouette onto what is behind
//! it); the **gather** blurs every pixel along its neighbourhood's streak, weighting
//! each tap by its distance from the camera and by the tap's own streak. Its output,
//! the blurred HDR scene, is what bloom and the composite read
//! ([`super::PostFx::scene_color`]). A viewmodel's gun writes ~0 velocity and lies
//! nearer than the world, so the world's streaks never smear over it and it stays
//! sharp. The distance comes from the velocity target (each camera writes its own),
//! not the depth buffer, which a `DepthOnly` overlay has cleared by now.
//!
//! **One path.** Camera motion is already in the velocity target (the sky carries
//! the camera's rotation), so there is no separate camera-only blur. A view with no
//! velocity target this frame (motion blur off, the Low tier, the player's option, a
//! shader preview, the editor's Scene view) blurs nothing and holds no blur targets.
//!
//! The pipelines are the renderer's ([`MotionBlurPasses`], built on first use and
//! warmed with the rest); the targets are each view's ([`BlurTargets`]).

mod params;

pub(crate) use params::BlurParams;

use crate::render::gpu::fullscreen::Fullscreen;
use crate::render::gpu::layout_entries::{texture_entry, uniform_entry};
use crate::render::gpu::pipelines::LazyPipeline;
use crate::render::gpu::shaders::ShaderRegistry;
use crate::render::postfx::{PfxTarget, PostFx, HDR_FORMAT};
use crate::render::timing::{GpuPass, GpuTimer};

/// The tile textures' format: a streak in pixels.
const TILE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rg16Float;

/// The motion-blur pipelines, shared by every view.
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

    /// The pipelines, for warming (`Renderer::lazy_pipelines`).
    pub(crate) fn lazy_pipelines(&self) -> [&LazyPipeline; 4] {
        [
            &self.tile_max_x,
            &self.tile_max_y,
            &self.neighbor_max,
            &self.gather,
        ]
    }
}

/// One view's motion-blur targets: the blurred scene and the params uniform, kept
/// while the view's size holds, and the tile textures, kept while the tile size
/// does too (a clamp ramp rebuilds only those). Held only while the view blurs.
pub(crate) struct BlurTargets {
    /// The full size these were made for.
    size: (u32, u32),
    /// The blurred HDR scene bloom and the composite read.
    pub(crate) blurred: PfxTarget,
    params: wgpu::Buffer,
    tiles: TileTargets,
}

/// The tile textures at one tile size: TileMax's half-way rows (tile columns by
/// pixel rows), TileMax and NeighborMax.
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

    /// Fit these to a view of `size` at `tile`: remade at another size, only the
    /// tiles remade at another tile size.
    fn fit(slot: &mut Option<Self>, device: &wgpu::Device, size: (u32, u32), tile: u32) {
        match slot {
            Some(t) if t.size == size => {
                if t.tiles.tile != tile {
                    t.tiles = TileTargets::new(device, size, tile);
                }
            }
            _ => *slot = Some(Self::new(device, size, tile)),
        }
    }
}

impl PostFx {
    /// Blur this frame's scene with `params`, or drop the blur targets when `params`
    /// is `None` or the view wrote no velocity this frame. Afterwards
    /// [`PostFx::scene_color`] is the image bloom and the composite read.
    pub(crate) fn run_motion_blur(
        &mut self,
        (device, queue): (&wgpu::Device, &wgpu::Queue),
        passes: &MotionBlurPasses,
        params: Option<BlurParams>,
        timer: &GpuTimer,
    ) {
        let Some((params, velocity)) = params.zip(self.motion.target_view()) else {
            self.blur = None;
            return;
        };
        BlurTargets::fit(&mut self.blur, device, self.full_size, params.tile());
        let targets = self.blur.as_ref().expect("fitted above");
        let tiles = &targets.tiles;
        queue.write_buffer(&targets.params, 0, bytemuck::bytes_of(&params));
        let group = |input: &wgpu::TextureView| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Motion Blur Bind Group"),
                layout: &passes.layout,
                entries: &[
                    entry(0, targets.params.as_entire_binding()),
                    entry(1, wgpu::BindingResource::TextureView(self.focused_color())),
                    entry(2, wgpu::BindingResource::TextureView(velocity)),
                    entry(3, wgpu::BindingResource::TextureView(input)),
                ],
            })
        };
        // Each pass reads the one before it as its tiles (binding 3); the first
        // reads none, and binds a texture it does not write.
        let steps = [
            (&passes.tile_max_x, &tiles.neighbor_max, &tiles.rows),
            (&passes.tile_max_y, &tiles.rows, &tiles.tile_max),
            (&passes.neighbor_max, &tiles.tile_max, &tiles.neighbor_max),
            (&passes.gather, &tiles.neighbor_max, &targets.blurred),
        ];
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Motion Blur Encoder"),
        });
        for (pipeline, input, target) in steps {
            let timed = (pipeline.get(), timer.writes(GpuPass::MotionBlur));
            Self::fullscreen(&mut encoder, timed, &[&group(&input.view)], &target.view);
        }
        queue.submit(std::iter::once(encoder.finish()));
    }
}

fn entry(binding: u32, resource: wgpu::BindingResource<'_>) -> wgpu::BindGroupEntry<'_> {
    wgpu::BindGroupEntry { binding, resource }
}

#[cfg(test)]
mod gpu_tests;

#[cfg(test)]
mod overlay_gpu_tests;

#[cfg(test)]
mod stack_order_gpu_tests;
