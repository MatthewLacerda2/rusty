//! src/render/ui/effects/mask_gpu.rs — each `Mask`'s coverage texture (#428).
//!
//! The mask-texture approach rather than stencil: a Mask's graphic is drawn into an
//! R8 texture the size of its canvas's frame (the screen, or a world canvas's own
//! rect), cleared to 0, through `ui.wgsl`'s `fs_mask` — the graphic's alpha times
//! the clip of every mask above it, which that draw samples. So a mask's texture is
//! already the product of its whole chain, a masked batch samples exactly one
//! texture, and nesting works to any depth with no stencil bits to run out of.
//! Coverage is continuous, which is what makes a soft-edged sprite a feathered
//! mask; a stencil could only cut hard. The canvas-NDC addressing is shared with
//! the batches that sample it, so the same texture serves an overlay and a world
//! canvas. Rendered once per frame before the camera stack — a mask does not
//! depend on the camera.

use std::collections::HashMap;

use super::color_pass;
use super::GroupKey;
use crate::render::{RenderView, Renderer};

/// The coverage textures' format: one 8-bit channel.
pub(crate) const MASK_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R8Unorm;

/// A view's Mask textures, by Mask entity.
#[derive(Default)]
pub(crate) struct MaskTargets {
    targets: HashMap<u32, (wgpu::Texture, wgpu::TextureView)>,
}

impl MaskTargets {
    /// Mask `id`'s coverage texture, if it was rendered this frame.
    pub(crate) fn view(&self, id: u32) -> Option<&wgpu::TextureView> {
        self.targets.get(&id).map(|(_, v)| v)
    }

    /// Keep a target for exactly the `(id, size)`s in `wanted`, reallocating any
    /// whose size changed.
    fn sync(&mut self, device: &wgpu::Device, wanted: &[(u32, (u32, u32))]) {
        let mut old = std::mem::take(&mut self.targets);
        for &(id, (w, h)) in wanted {
            let target = match old.remove(&id) {
                Some((t, v)) if (t.width(), t.height()) == (w, h) => (t, v),
                _ => create(device, w, h),
            };
            self.targets.insert(id, target);
        }
    }
}

fn create(device: &wgpu::Device, w: u32, h: u32) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("UI Mask"),
        size: wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: MASK_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view)
}

/// The pipeline drawing a Mask's graphic into its texture: the UI vertices,
/// `fs_mask`, max-blended (a graphic's own triangles never lower its coverage).
pub(crate) fn mask_pipeline(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    layout: &wgpu::PipelineLayout,
) -> wgpu::RenderPipeline {
    let max = wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::One,
        dst_factor: wgpu::BlendFactor::One,
        operation: wgpu::BlendOperation::Max,
    };
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("UI Mask Pipeline"),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: "vs_main",
            buffers: &[crate::render::ui::vertex::vertex_layout()],
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: "fs_mask",
            targets: &[Some(wgpu::ColorTargetState {
                format: MASK_FORMAT,
                blend: Some(wgpu::BlendState {
                    color: max,
                    alpha: max,
                }),
                write_mask: wgpu::ColorWrites::RED,
            })],
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
    })
}

impl Renderer {
    /// Render every synced canvas's Mask textures, parents before children (see the
    /// module docs). Needs the batch uniform written first.
    pub(crate) fn render_masks(&self, view: &mut RenderView) {
        let cache = &mut view.ui;
        let wanted = mask_sizes(cache, self.device.limits().max_texture_dimension_2d);
        cache.effects.mask_passes = wanted.len() as u32;
        cache.effects.masks.sync(&self.device, &wanted);
        if wanted.is_empty() {
            return;
        }
        let key = |m: &super::mask::MaskDraw| GroupKey {
            mask: m.clip.mask,
            backdrop: None,
        };
        let keys = cache
            .canvases()
            .iter()
            .flat_map(|c| c.mesh.masks.iter().map(key));
        let groups = self.batch_groups(&cache.effects, keys);
        let ui = &self.ui_renderer;
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("UI Mask Encoder"),
            });
        for (i, canvas) in cache.canvases().iter().enumerate() {
            for (m, draw) in canvas.mesh.masks.iter().enumerate() {
                let Some(target) = cache.effects.masks.view(draw.id) else {
                    continue;
                };
                let clear = wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT);
                let mut pass = color_pass(&mut encoder, "UI Mask Pass", target, clear);
                pass.set_pipeline(&ui.mask_pipeline);
                pass.set_vertex_buffer(0, canvas.buffer.slice(..));
                pass.set_bind_group(0, ui.source_group(&draw.source), &[]);
                let slot = cache.effects.mask_offset(i, m);
                pass.set_bind_group(1, &groups[&key(draw)], &[slot]);
                pass.draw(draw.range.clone(), 0..1);
            }
        }
        self.queue.submit(Some(encoder.finish()));
    }
}

/// Every synced Mask and its texture size: its canvas's frame, clamped to the
/// device's limit.
fn mask_sizes(cache: &crate::render::ui::UiViewCache, limit: u32) -> Vec<(u32, (u32, u32))> {
    let size = |f: glam::Vec2| {
        let s = f
            .ceil()
            .clamp(glam::Vec2::ONE, glam::Vec2::splat(limit as f32));
        (s.x as u32, s.y as u32)
    };
    let canvases = cache.canvases().iter();
    canvases
        .flat_map(|c| c.mesh.masks.iter().map(|m| (m.id, size(c.mesh.frame))))
        .collect()
}
