//! src/render/postfx/motion_blur/ — per-object motion blur (#1436): the
//! reconstruction filter over a view's velocity target (#1435), in
//! `postfx/motion_blur.wgsl`.
//!
//! Three fullscreen passes, before the composite: **TileMax** keeps each tile's
//! longest streak, **NeighborMax** spreads each tile's to its 3×3 neighbourhood (so
//! the blur reaches past a moving silhouette onto what is behind it), and the
//! **gather** blurs every pixel along its neighbourhood's streak, weighting each tap
//! by depth and by the tap's own streak. Its output, the blurred HDR scene, is what
//! the composite reads ([`super::PostFx::scene_color`]). A viewmodel's gun writes
//! ~0 velocity and sits in front of the world, so the world's streaks never smear
//! over it and it stays sharp.
//!
//! **One path.** Camera motion is already in the velocity target (the sky carries
//! the camera's rotation), so there is no separate camera-only blur. A view with no
//! velocity target this frame (motion blur off, the Low tier, the player's option, a
//! shader preview) blurs nothing and holds no blur targets.
//!
//! The pipelines are the renderer's ([`MotionBlurPasses`], built on first use and
//! warmed with the rest); the targets are each view's ([`BlurTargets`]).

mod params;

pub(crate) use params::BlurParams;

use crate::render::gpu::fullscreen::Fullscreen;
use crate::render::gpu::layout_entries::{depth_entry, texture_entry, uniform_entry};
use crate::render::gpu::pipelines::LazyPipeline;
use crate::render::gpu::shaders::ShaderRegistry;
use crate::render::motion::VELOCITY_FORMAT;
use crate::render::postfx::{PfxTarget, PostFx, HDR_FORMAT};
use crate::render::timing::{GpuPass, GpuTimer};

/// The motion-blur pipelines, shared by every view.
pub(crate) struct MotionBlurPasses {
    layout: wgpu::BindGroupLayout,
    tile_max: LazyPipeline,
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
                depth_entry(4, wgpu::TextureViewDimension::D2),
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
            tile_max: pass("fs_tile_max", VELOCITY_FORMAT),
            neighbor_max: pass("fs_neighbor_max", VELOCITY_FORMAT),
            gather: pass("fs_gather", HDR_FORMAT),
            layout,
        }
    }

    /// The pipelines, for warming (`Renderer::lazy_pipelines`).
    pub(crate) fn lazy_pipelines(&self) -> [&LazyPipeline; 3] {
        [&self.tile_max, &self.neighbor_max, &self.gather]
    }
}

/// One view's motion-blur targets: the two tile textures, the blurred scene and
/// the params uniform. Held only while the view blurs.
pub(crate) struct BlurTargets {
    /// The full size and tile size these were made for.
    key: ((u32, u32), u32),
    tile_max: PfxTarget,
    neighbor_max: PfxTarget,
    /// The blurred HDR scene the composite reads.
    pub(crate) blurred: PfxTarget,
    params: wgpu::Buffer,
}

impl BlurTargets {
    fn new(device: &wgpu::Device, (w, h): (u32, u32), tile: u32) -> Self {
        let (tw, th) = (params::tiles(w, tile), params::tiles(h, tile));
        Self {
            key: ((w, h), tile),
            tile_max: PostFx::target(device, tw, th, "Motion Blur TileMax", VELOCITY_FORMAT),
            neighbor_max: PostFx::target(
                device,
                tw,
                th,
                "Motion Blur NeighborMax",
                VELOCITY_FORMAT,
            ),
            blurred: PostFx::target(device, w, h, "Motion Blur Scene", HDR_FORMAT),
            params: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Motion Blur Params"),
                size: std::mem::size_of::<BlurParams>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
        }
    }
}

impl PostFx {
    /// Blur this frame's scene with `params`, or drop the blur targets when `params`
    /// is `None` or the view wrote no velocity this frame. Afterwards
    /// [`PostFx::scene_color`] is the image the composite reads.
    pub(crate) fn run_motion_blur(
        &mut self,
        (device, queue): (&wgpu::Device, &wgpu::Queue),
        passes: &MotionBlurPasses,
        depth: &wgpu::TextureView,
        params: Option<BlurParams>,
        timer: &GpuTimer,
    ) {
        let Some((params, velocity)) = params.zip(self.motion.target_view()) else {
            self.blur = None;
            return;
        };
        let key = (self.full_size, params.tile());
        if self.blur.as_ref().is_none_or(|t| t.key != key) {
            self.blur = Some(BlurTargets::new(device, key.0, key.1));
        }
        let targets = self.blur.as_ref().expect("allocated above");
        queue.write_buffer(&targets.params, 0, bytemuck::bytes_of(&params));
        let group = |tiles: &wgpu::TextureView| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Motion Blur Bind Group"),
                layout: &passes.layout,
                entries: &[
                    entry(0, targets.params.as_entire_binding()),
                    entry(1, wgpu::BindingResource::TextureView(&self.scene_hdr.view)),
                    entry(2, wgpu::BindingResource::TextureView(velocity)),
                    entry(3, wgpu::BindingResource::TextureView(tiles)),
                    entry(4, wgpu::BindingResource::TextureView(depth)),
                ],
            })
        };
        // Each pass binds as its tiles a texture it does not write.
        let steps = [
            (
                &passes.tile_max,
                group(&targets.neighbor_max.view),
                &targets.tile_max,
            ),
            (
                &passes.neighbor_max,
                group(&targets.tile_max.view),
                &targets.neighbor_max,
            ),
            (
                &passes.gather,
                group(&targets.neighbor_max.view),
                &targets.blurred,
            ),
        ];
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Motion Blur Encoder"),
        });
        for (pipeline, bind_group, target) in &steps {
            let timed = (pipeline.get(), timer.writes(GpuPass::MotionBlur));
            Self::fullscreen(&mut encoder, timed, &[bind_group], &target.view);
        }
        queue.submit(std::iter::once(encoder.finish()));
    }
}

fn entry(binding: u32, resource: wgpu::BindingResource<'_>) -> wgpu::BindGroupEntry<'_> {
    wgpu::BindGroupEntry { binding, resource }
}

#[cfg(test)]
mod gpu_tests;
