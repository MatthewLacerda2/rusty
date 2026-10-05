//! src/render/ui/effects/ — what clips and backs a UI batch: soft and shaped masks
//! (#428) and the frosted-glass backdrop (#426).
//!
//! Every batch, on an overlay or a world canvas, binds one **batch group** (group 1
//! of `ui.wgsl`): its slot in the view's batch uniform ([`BatchUniform`]: the
//! RectMask bounds and feather in canvas NDC, a backdrop's tint and filter), the
//! coverage texture of its nearest `Mask`, the blurred frame a backdrop shows, and
//! a sampler. Batches with no mask or backdrop bind the white texture there, so one
//! shader path serves them all. The slots are camera-independent — canvas NDC — so
//! they are written once per frame in `prepare_ui`, and the world pass reuses them
//! for every camera.
//!
//! - `mask` / `backdrop` — the pure halves the mesh builder calls.
//! - `mask_gpu` — each Mask's coverage texture, rendered before the camera stack.
//! - `blur` — the dual-filter chain over the finished frame, run before the
//!   overlay pass.

pub(crate) mod backdrop;
pub(crate) mod blur;
pub(crate) mod mask;
pub(crate) mod mask_gpu;

use std::collections::HashMap;

use glam::Vec2;

use self::backdrop::UiBackdrop;
use super::cache::UiViewCache;
use super::mesh::{UiBatch, UiClip};
use crate::render::gpu::grow_buffer::GrowBuffer;
use crate::render::timing::{GpuPass, GpuTimer};
use crate::render::Renderer;

/// One batch's slot of the view's batch uniform — matches `ui.wgsl`'s `UiBatch`.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct BatchUniform {
    clip: [f32; 4],
    feather: [f32; 4],
    tint: [f32; 4],
    grade: [f32; 4],
}

impl BatchUniform {
    /// The slot for a batch clipped by `clip` on a `frame`-pixel canvas, showing
    /// `backdrop` (if any).
    pub(crate) fn new(clip: &UiClip, frame: Vec2, backdrop: Option<&UiBackdrop>) -> Self {
        let (rect, feather) = clip.ndc(frame);
        let (tint, [s, b]) = backdrop.map_or(([0.0; 4], [1.0, 1.0]), |d| (d.tint, d.filter));
        Self {
            clip: rect,
            feather,
            tint,
            grade: [s, b, 0.0, 0.0],
        }
    }
}

/// Which batch group a batch binds: its mask (by Mask entity) and its backdrop's
/// blur level.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) struct GroupKey {
    pub(crate) mask: Option<u32>,
    pub(crate) backdrop: Option<u32>,
}

impl GroupKey {
    /// `batch`'s key. `divisor` is the blur's (the overlay pass); `None` drops the
    /// backdrop (the world pass never draws one).
    pub(crate) fn of(batch: &UiBatch, divisor: Option<u32>) -> Self {
        let level = |d: u32| batch.backdrop.map(|b| backdrop::blur_level(b.radius, d));
        Self {
            mask: batch.clip.mask,
            backdrop: divisor.and_then(level),
        }
    }
}

/// A view's effect state: the batch uniform and where each canvas's slots start,
/// the Mask textures, the blur chain, and the last frame's pass counts.
#[derive(Default)]
pub(crate) struct UiEffects {
    uniforms: Option<GrowBuffer>,
    /// Bytes between two slots (the dynamic-offset alignment).
    stride: u32,
    /// Each synced canvas's first batch slot, then its first mask-draw slot.
    slots: Vec<(u32, u32)>,
    pub(crate) masks: mask_gpu::MaskTargets,
    pub(crate) blur: Option<blur::BlurTargets>,
    /// Mask textures rendered by the last frame (#428).
    pub(crate) mask_passes: u32,
    /// Fullscreen blur passes run by the last frame, the backdrop's composite
    /// included (#426).
    pub(crate) blur_passes: u32,
}

impl UiEffects {
    /// The dynamic offset of canvas `i`'s batch `b`.
    pub(crate) fn batch_offset(&self, i: usize, b: usize) -> u32 {
        (self.slots[i].0 + b as u32) * self.stride
    }

    /// The dynamic offset of canvas `i`'s mask draw `m`.
    pub(crate) fn mask_offset(&self, i: usize, m: usize) -> u32 {
        (self.slots[i].1 + m as u32) * self.stride
    }
}

/// The batch group's layout: the dynamic uniform, the mask and backdrop textures,
/// and their sampler.
pub(crate) fn batch_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    let texture = |binding| wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    };
    let uniform = wgpu::BindGroupLayoutEntry {
        binding: 0,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: true,
            min_binding_size: wgpu::BufferSize::new(SLOT),
        },
        count: None,
    };
    let sampler = wgpu::BindGroupLayoutEntry {
        binding: 3,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
        count: None,
    };
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("UI Batch Layout"),
        entries: &[uniform, texture(1), texture(2), sampler],
    })
}

/// A colour-only render pass over `target`, loading or clearing it — the overlay,
/// mask and blur passes, timed as `ui` (#835).
pub(crate) fn color_pass<'e>(
    encoder: &'e mut wgpu::CommandEncoder,
    (label, timer): (&str, &GpuTimer),
    target: &'e wgpu::TextureView,
    load: wgpu::LoadOp<wgpu::Color>,
) -> wgpu::RenderPass<'e> {
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        multiview_mask: None,
        label: Some(label),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            depth_slice: None,
            view: target,
            resolve_target: None,
            ops: wgpu::Operations {
                load,
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: timer.writes(GpuPass::Ui),
        occlusion_query_set: None,
    })
}

/// One slot's size in bytes.
const SLOT: u64 = std::mem::size_of::<BatchUniform>() as u64;

impl Renderer {
    /// Write every synced canvas's batch and mask-draw slots into `cache`'s batch
    /// uniform (see the module docs).
    pub(crate) fn upload_batch_uniforms(&self, cache: &mut UiViewCache) {
        let align = self.device.limits().min_uniform_buffer_offset_alignment;
        let stride = (SLOT as u32).next_multiple_of(align);
        let mut slots = Vec::new();
        let mut bytes: Vec<u8> = Vec::new();
        let mut push = |u: BatchUniform| {
            bytes.extend_from_slice(bytemuck::bytes_of(&u));
            bytes.resize(bytes.len() + (stride as usize - SLOT as usize), 0);
        };
        let mut next = 0u32;
        for c in cache.canvases() {
            let mesh = &c.mesh;
            let first_mask = next + mesh.batches.len() as u32;
            slots.push((next, first_mask));
            next = first_mask + mesh.masks.len() as u32;
            for b in &mesh.batches {
                push(BatchUniform::new(&b.clip, mesh.frame, b.backdrop.as_ref()));
            }
            for m in &mesh.masks {
                push(BatchUniform::new(&m.clip, mesh.frame, None));
            }
        }
        // An empty view still binds a slot (a zero-sized binding is invalid).
        bytes.resize(bytes.len().max(stride as usize), 0);
        let fx = &mut cache.effects;
        let usage = wgpu::BufferUsages::UNIFORM;
        let buffer = fx
            .uniforms
            .get_or_insert_with(|| GrowBuffer::new(&self.device, "UI Batch Uniforms", usage));
        buffer.upload(&self.device, &self.queue, &bytes);
        (fx.stride, fx.slots) = (stride, slots);
    }

    /// The batch group of every key in `keys`, against `fx`'s current uniform buffer.
    pub(crate) fn batch_groups(
        &self,
        fx: &UiEffects,
        keys: impl IntoIterator<Item = GroupKey>,
    ) -> HashMap<GroupKey, wgpu::BindGroup> {
        let ui = &self.ui_renderer;
        let mut groups = HashMap::new();
        let Some(buffer) = fx.uniforms.as_ref() else {
            return groups;
        };
        for key in keys {
            if groups.contains_key(&key) {
                continue;
            }
            let mask = key.mask.and_then(|m| fx.masks.view(m));
            let blurred = key.backdrop.and_then(|l| fx.blur.as_ref()?.result(l));
            let entries = [
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: buffer.buffer(),
                        offset: 0,
                        size: wgpu::BufferSize::new(SLOT),
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(mask.unwrap_or(&ui.white_view)),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(blurred.unwrap_or(&ui.white_view)),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&ui.sampler),
                },
            ];
            let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("UI Batch Group"),
                layout: &ui.batch_layout,
                entries: &entries,
            });
            groups.insert(key, group);
        }
        groups
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
