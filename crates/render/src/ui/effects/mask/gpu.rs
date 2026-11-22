use std::collections::HashMap;

use super::key::{DrawnInputs, MaskInputs};
use crate::gpu::fullscreen::color_pass;
use crate::render_texture::is_render_texture;
use crate::timing::GpuPass;
use crate::ui::cache::{CanvasGpu, UiViewCache};
use crate::ui::effects::{BatchUniform, GroupKey};
use crate::ui::mesh::UiSource;
use crate::ui::pipeline::{BATCH_GROUP, SOURCE_GROUP};
use crate::{RenderView, Renderer};

pub(crate) const MASK_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R8Unorm;

struct MaskTarget {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    drawn: Option<DrawnInputs>,
    stamp: u64,
}

#[derive(Default)]
pub(crate) struct MaskTargets {
    targets: HashMap<u32, MaskTarget>,
    stamps: u64,
}

impl MaskTargets {
    pub(crate) fn view(&self, id: u32) -> Option<&wgpu::TextureView> {
        self.targets.get(&id).map(|t| &t.view)
    }

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
    /// After `upload_batch_uniforms` (no buffer, no groups). Its `forget_groups` opens the frame's
    /// groups.
    pub(crate) fn render_masks(&self, encoder: &mut wgpu::CommandEncoder, view: &mut RenderView) {
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
        let key = |m: &super::MaskDraw| GroupKey {
            mask: m.clip.mask,
            backdrop: None,
        };
        let keys: Vec<_> = draws.iter().map(|&d| key(draw_of(cache, d))).collect();
        self.batch_groups(&mut cache.effects, keys);
        let cache = &*cache;
        let ui = &self.ui_renderer;
        for (i, m) in draws {
            let (canvas, draw) = (cache.canvas(i), draw_of(cache, (i, m)));
            let Some(target) = cache.effects.masks.view(draw.id) else {
                continue;
            };
            let clear = wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT);
            let mut pass = color_pass(
                encoder,
                ("UI Mask Pass", &self.gpu_timer, GpuPass::Ui),
                target,
                clear,
            );
            pass.set_pipeline(&ui.mask_pipeline);
            pass.set_vertex_buffer(0, canvas.buffer.slice(..));
            pass.set_bind_group(SOURCE_GROUP, ui.source_group(&draw.source), &[]);
            let slot = cache.effects.mask_offset(i, m);
            pass.set_bind_group(BATCH_GROUP, cache.effects.group(&key(draw)), &[slot]);
            pass.draw(draw.range.clone(), 0..1);
        }
        self.lap("ui.masks");
    }

    /// In `CanvasMesh::masks` order, parents first: a child's inputs read its parent's new stamp.
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

    fn mask_inputs<'a>(
        &'a self,
        masks: &MaskTargets,
        canvas: &'a CanvasGpu,
        draw: &super::MaskDraw,
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

fn draw_of(cache: &UiViewCache, (i, m): (usize, usize)) -> &super::MaskDraw {
    &cache.canvas(i).mesh.masks[m]
}

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
