pub(crate) mod backdrop;
pub(crate) mod blur;
pub(crate) mod mask;

use crate::gpu::ops::CountedDevice as _;
use std::collections::{HashMap, HashSet};

use glam::Vec2;

use self::backdrop::UiBackdrop;
use super::cache::UiViewCache;
use super::mesh::{UiBatch, UiClip};
use crate::gpu::grow_buffer::GrowBuffer;
use crate::gpu::kept::Kept;
use crate::gpu::layout_entries::{dynamic_uniform_entry, sampler_entry, texture_entry};
use crate::Renderer;

/// One batch's slot of the batch uniform; mirrors `ui.wgsl`'s `UiBatch`: change both together.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct BatchUniform {
    clip: [f32; 4],
    feather: [f32; 4],
    tint: [f32; 4],
    grade: [f32; 4],
}

impl BatchUniform {
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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct GroupKey {
    pub(crate) mask: Option<u32>,
    pub(crate) backdrop: Option<u32>,
}

impl GroupKey {
    pub(crate) fn of(batch: &UiBatch, divisor: Option<u32>) -> Self {
        let level = |d: u32| batch.backdrop.map(|b| backdrop::blur_level(b.radius, d));
        Self {
            mask: batch.clip.mask,
            backdrop: divisor.and_then(level),
        }
    }
}

type GroupInputs = (
    wgpu::Buffer,
    Option<wgpu::TextureView>,
    Option<wgpu::TextureView>,
);

#[derive(Default)]
pub(crate) struct UiEffects {
    uniforms: Option<GrowBuffer>,
    uploaded: Vec<u8>,
    pub(crate) uniform_uploads: u32,
    groups: HashMap<GroupKey, Kept<GroupInputs, wgpu::BindGroup>>,
    asked: HashSet<GroupKey>,
    stride: u32,
    slots: Vec<(u32, u32)>,
    pub(crate) masks: mask::gpu::MaskTargets,
    pub(crate) blur: Option<blur::BlurTargets>,
    pub(crate) mask_passes: u32,
    pub(crate) blur_passes: u32,
}

impl UiEffects {
    pub(crate) fn batch_offset(&self, i: usize, b: usize) -> u32 {
        (self.slots[i].0 + b as u32) * self.stride
    }

    pub(crate) fn mask_offset(&self, i: usize, m: usize) -> u32 {
        (self.slots[i].1 + m as u32) * self.stride
    }

    pub(crate) fn group(&self, key: &GroupKey) -> &wgpu::BindGroup {
        let kept = self.groups.get(key).and_then(Kept::value);
        kept.expect("batch_groups made every key's group")
    }

    pub(crate) fn forget_groups(&mut self) {
        let asked = std::mem::take(&mut self.asked);
        self.groups.retain(|k, _| asked.contains(k));
    }

    #[cfg(test)]
    pub(crate) fn group_count(&self) -> usize {
        self.groups.len()
    }
}

pub(crate) fn batch_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    let uniform = dynamic_uniform_entry(0, wgpu::ShaderStages::FRAGMENT, SLOT);
    let sampler = sampler_entry(3);
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("UI Batch Layout"),
        entries: &[
            uniform,
            texture_entry(1, wgpu::TextureViewDimension::D2),
            texture_entry(2, wgpu::TextureViewDimension::D2),
            sampler,
        ],
    })
}

const SLOT: u64 = std::mem::size_of::<BatchUniform>() as u64;

impl Renderer {
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
        // An empty view still binds one slot: a zero-sized binding is invalid.
        bytes.resize(bytes.len().max(stride as usize), 0);
        let fx = &mut cache.effects;
        (fx.stride, fx.slots) = (stride, slots);
        fx.uniform_uploads = 0;
        if fx.uniforms.is_some() && fx.uploaded == bytes {
            return;
        }
        let usage = wgpu::BufferUsages::UNIFORM;
        let buffer = fx
            .uniforms
            .get_or_insert_with(|| GrowBuffer::new(&self.device, "UI Batch Uniforms", usage));
        buffer.upload(&self.device, &self.queue, &bytes);
        (fx.uploaded, fx.uniform_uploads) = (bytes, 1);
    }

    pub(crate) fn batch_groups(
        &self,
        fx: &mut UiEffects,
        keys: impl IntoIterator<Item = GroupKey>,
    ) {
        let Some(buffer) = fx.uniforms.as_ref().map(GrowBuffer::buffer) else {
            return;
        };
        let mut keys: Vec<GroupKey> = keys.into_iter().collect();
        keys.sort_unstable();
        keys.dedup();
        for key in keys {
            fx.asked.insert(key);
            let mask = key.mask.and_then(|m| fx.masks.view(m));
            let blurred = key.backdrop.and_then(|l| fx.blur.as_ref()?.result(l));
            let kept = fx.groups.entry(key).or_default();
            let current = kept.key().is_some_and(|(b, m, r)| {
                b == buffer && m.as_ref() == mask && r.as_ref() == blurred
            });
            if current {
                continue;
            }
            let inputs = (buffer.clone(), mask.cloned(), blurred.cloned());
            let group = self.batch_group(&inputs);
            kept.get(inputs, || group);
        }
    }

    fn batch_group(&self, (buffer, mask, blurred): &GroupInputs) -> wgpu::BindGroup {
        let ui = &self.ui_renderer;
        let entries = [
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer,
                    offset: 0,
                    size: wgpu::BufferSize::new(SLOT),
                }),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(
                    mask.as_ref().unwrap_or(&ui.white_view),
                ),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::TextureView(
                    blurred.as_ref().unwrap_or(&ui.white_view),
                ),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::Sampler(&ui.sampler),
            },
        ];
        self.device.counted_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("UI Batch Group"),
            layout: &ui.batch_layout,
            entries: &entries,
        })
    }
}

#[cfg(test)]
#[path = "chain_gpu_tests.rs"]
mod chain_gpu_tests;
#[cfg(test)]
#[path = "gpu_fixture.rs"]
mod gpu_fixture;
#[cfg(test)]
#[path = "gpu_tests.rs"]
mod gpu_tests;
#[cfg(test)]
#[path = "groups_gpu_tests.rs"]
mod groups_gpu_tests;
#[cfg(test)]
#[path = "tests.rs"]
mod tests;
