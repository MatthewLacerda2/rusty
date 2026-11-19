//! crates/render/src/ui/effects/mask_gpu.rs — each `Mask`'s coverage texture (#428).
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
//! canvas. Prepared once per frame before the camera stack — a mask does not
//! depend on the camera.
//!
//! **Change-driven.** A Mask's texture is re-rendered only when what its coverage
//! depends on changed ([`MaskInputs`]): its own vertices (not its canvas's, so a
//! minimap's turning map leaves its still frame alone), its slot of the batch
//! uniform, its source texture, its target, and which draw of which parent Mask it
//! sampled (a stamp no other draw of any Mask repeats, so a Mask added, removed or
//! moved above it redraws it even when every Mask involved is still). A
//! graphic whose source changes in place (a render texture, a glyph atlas) has no
//! such key and is re-rendered every frame. With nothing to re-render, no encoder
//! is made and nothing is submitted; `UiEffects::mask_passes` counts the passes run.

use crate::gpu::ops::CountedQueue as _;
use std::collections::HashMap;

use super::mask_key::{DrawnInputs, MaskInputs};
use super::{BatchUniform, GroupKey};
use crate::gpu::fullscreen::color_pass;
use crate::render_texture::is_render_texture;
use crate::timing::GpuPass;
use crate::ui::cache::{CanvasGpu, UiViewCache};
use crate::ui::mesh::UiSource;
use crate::{RenderView, Renderer};

/// The coverage textures' format: one 8-bit channel.
pub(crate) const MASK_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R8Unorm;

/// One Mask's coverage texture, the inputs it was last drawn from (`None`: it
/// holds no reusable draw) and the stamp of that draw.
struct MaskTarget {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    drawn: Option<DrawnInputs>,
    stamp: u64,
}

/// A view's Mask textures, by Mask entity.
#[derive(Default)]
pub(crate) struct MaskTargets {
    targets: HashMap<u32, MaskTarget>,
    /// The last stamp given to a draw: it only goes up, so no two draws of any
    /// Masks share one, and a child's key names which parent draw it sampled.
    stamps: u64,
}

impl MaskTargets {
    /// Mask `id`'s coverage texture, if it is synced this frame.
    pub(crate) fn view(&self, id: u32) -> Option<&wgpu::TextureView> {
        self.targets.get(&id).map(|t| &t.view)
    }

    /// Keep a target for exactly the `(id, size)`s in `wanted`, reallocating any
    /// whose size changed (a new target holds no draw).
    fn sync(&mut self, device: &wgpu::Device, wanted: &[(u32, (u32, u32))]) {
        let mut old = std::mem::take(&mut self.targets);
        for &(id, (w, h)) in wanted {
            let target = match old.remove(&id) {
                Some(t) if (t.texture.width(), t.texture.height()) == (w, h) => t,
                _ => create(device, w, h),
            };
            self.targets.insert(id, target);
        }
    }

    /// Whether Mask `id` must be drawn from `inputs` (`None`: always), recording
    /// the draw and stamping it when it must. Called parents first, so a child
    /// sees its parent's stamp after this frame's draw.
    fn needs_draw(&mut self, id: u32, inputs: Option<MaskInputs>) -> bool {
        let Some(target) = self.targets.get_mut(&id) else {
            return false;
        };
        match (&mut target.drawn, &inputs) {
            (Some(drawn), Some(i)) if drawn.matches(i) => return false,
            (Some(drawn), Some(i)) => drawn.hold(i),
            (drawn, Some(i)) => drawn.get_or_insert_default().hold(i),
            (drawn, None) => *drawn = None,
        }
        self.stamps += 1;
        target.stamp = self.stamps;
        true
    }

    /// The stamp of Mask `id`'s last draw, for its children's inputs (0: never
    /// drawn, which no draw is stamped).
    fn stamp(&self, id: u32) -> Option<u64> {
        self.targets.get(&id).map(|t| t.stamp)
    }
}

fn create(device: &wgpu::Device, w: u32, h: u32) -> MaskTarget {
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
    MaskTarget {
        texture,
        view,
        drawn: None,
        stamp: 0,
    }
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
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[Some(crate::ui::vertex::vertex_layout())],
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some("fs_mask"),
            compilation_options: Default::default(),
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
        multiview_mask: None,
        cache: None,
    })
}

impl Renderer {
    /// Render the Mask textures whose inputs changed, parents before children (see
    /// the module docs). Needs the batch uniform written first.
    pub(crate) fn render_masks(&self, view: &mut RenderView) {
        let cache = &mut view.ui;
        let wanted = mask_sizes(cache, self.device.limits().max_texture_dimension_2d);
        cache.effects.masks.sync(&self.device, &wanted);
        cache.effects.forget_groups();
        let draws = self.masks_to_draw(cache);
        cache.effects.mask_passes = draws.len() as u32;
        if draws.is_empty() {
            self.lap("ui.masks");
            return;
        }
        let key = |m: &super::mask::MaskDraw| GroupKey {
            mask: m.clip.mask,
            backdrop: None,
        };
        let keys: Vec<_> = draws.iter().map(|&d| key(draw_of(cache, d))).collect();
        self.batch_groups(&mut cache.effects, keys);
        let cache = &*cache;
        let ui = &self.ui_renderer;
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("UI Mask Encoder"),
            });
        for (i, m) in draws {
            let (canvas, draw) = (cache.canvas(i), draw_of(cache, (i, m)));
            let Some(target) = cache.effects.masks.view(draw.id) else {
                continue;
            };
            let clear = wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT);
            let mut pass = color_pass(
                &mut encoder,
                ("UI Mask Pass", &self.gpu_timer, GpuPass::Ui),
                target,
                clear,
            );
            pass.set_pipeline(&ui.mask_pipeline);
            pass.set_vertex_buffer(0, canvas.buffer.slice(..));
            pass.set_bind_group(0, ui.source_group(&draw.source), &[]);
            let slot = cache.effects.mask_offset(i, m);
            pass.set_bind_group(1, cache.effects.group(&key(draw)), &[slot]);
            pass.draw(draw.range.clone(), 0..1);
        }
        let commands = encoder.finish();
        self.lap("ui.masks");
        self.queue.counted_submit(Some(commands));
        self.lap("ui.mask_submit");
    }

    /// Every `(canvas, mask draw)` whose inputs changed, in order, each recorded
    /// as drawn.
    fn masks_to_draw(&self, cache: &mut UiViewCache) -> Vec<(usize, usize)> {
        let mut draws = Vec::new();
        let (canvases, effects) = cache.canvases_and_effects();
        for (i, canvas) in canvases.iter().enumerate() {
            for (m, draw) in canvas.mesh.masks.iter().enumerate() {
                let inputs = self.mask_inputs(&effects.masks, canvas, draw);
                if effects.masks.needs_draw(draw.id, inputs) {
                    draws.push((i, m));
                }
            }
        }
        draws
    }

    /// What `draw`'s coverage is drawn from, or `None` when its source can change
    /// in place (a render texture, a glyph atlas) and it must be drawn each frame.
    fn mask_inputs<'a>(
        &'a self,
        masks: &MaskTargets,
        canvas: &'a CanvasGpu,
        draw: &super::mask::MaskDraw,
    ) -> Option<MaskInputs<'a>> {
        let source = match &draw.source {
            UiSource::Solid => None,
            UiSource::Font(_) => return None,
            UiSource::Texture(p) if is_render_texture(p) => return None,
            UiSource::Texture(p) => self.ui_renderer.textures.get(p).map(|(t, _)| &t.view),
        };
        let range = draw.range.start as usize..draw.range.end as usize;
        let vertices = bytemuck::cast_slice(&canvas.mesh.vertices[range]);
        let parent = match draw.clip.mask {
            Some(id) => Some(masks.stamp(id)?),
            None => None,
        };
        Some(MaskInputs {
            vertices,
            slot: BatchUniform::new(&draw.clip, canvas.mesh.frame, None),
            source,
            parent,
        })
    }
}

/// Canvas `i`'s mask draw `m`.
fn draw_of(cache: &UiViewCache, (i, m): (usize, usize)) -> &super::mask::MaskDraw {
    &cache.canvas(i).mesh.masks[m]
}

/// Every synced Mask and its texture size: its canvas's frame, clamped to the
/// device's limit.
fn mask_sizes(cache: &crate::ui::UiViewCache, limit: u32) -> Vec<(u32, (u32, u32))> {
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
