//! src/render/gpu/material_cache.rs — one group-2 material bind group per distinct set
//! of resolved texture maps (#470), plus each material's runtime shader-param uniform
//! (#399).
//!
//! The per-entity pool (#210) built a material bind group per entity, so a hundred
//! crates held a hundred identical groups — and no two could share a draw call. Keyed
//! by the maps they bind instead, every crate resolves to the same index, which is
//! what lets [`BatchKey`](crate::render::draw::batch::BatchKey) group them.
//!
//! The key is the *resolved* signature (a map's path only once its texture is
//! resident, else empty for the default), so a late-loading texture gets a fresh group
//! rather than a stale one (#207). Entries are evicted only when a texture file is
//! re-written (#689): then every group is dropped and rebuilt on next use, since a
//! group holds the old upload. There is one per material look in use, a small set,
//! and each is only a bind group over shared textures.
//!
//! **Runtime params (#399).** A material whose surface shader exposes runtime params
//! owns one uniform buffer, keyed by its library name, and its group binds that buffer
//! — so its key also carries the name, and it batches only with itself. Every other
//! material binds the shared all-zero buffer and batches by maps exactly as before. A
//! param change is a `write_buffer` into the owned buffer: the group, the pipeline and
//! the WGSL are untouched.

use std::collections::HashMap;

use crate::shadergen::params::PackedParams;

/// Group-2 textures: the five maps plus one per extra shader texture slot (#400).
pub(crate) const MATERIAL_TEXTURES: usize = 5 + crate::shadergen::textures::SLOTS.len();

/// Albedo, metallic, roughness, normal and emissive map keys, then the extra shader
/// texture slots (`mask`), in that order.
pub(crate) type MapSignature = [String; MATERIAL_TEXTURES];

/// A group's identity: its maps, and the material owning its param buffer (if any).
pub(crate) type GroupKey = (MapSignature, Option<String>);

pub(crate) struct MaterialCache {
    index: HashMap<GroupKey, usize>,
    groups: Vec<wgpu::BindGroup>,
    /// The params every material without runtime params binds (all zero).
    zero_params: wgpu::Buffer,
    /// Each runtime-param material's buffer, with the values last written to it.
    params: HashMap<String, (wgpu::Buffer, PackedParams)>,
}

impl MaterialCache {
    pub(crate) fn new(zero_params: wgpu::Buffer) -> Self {
        Self {
            index: HashMap::new(),
            groups: Vec::new(),
            zero_params,
            params: HashMap::new(),
        }
    }

    /// The cache index for `key`, if a group was already built for it.
    pub(crate) fn lookup(&self, key: &GroupKey) -> Option<usize> {
        self.index.get(key).copied()
    }

    /// Store `group` for `key`, returning its index.
    pub(crate) fn insert(&mut self, key: GroupKey, group: wgpu::BindGroup) -> usize {
        let i = self.groups.len();
        self.groups.push(group);
        self.index.insert(key, i);
        i
    }

    pub(crate) fn group(&self, i: usize) -> &wgpu::BindGroup {
        &self.groups[i]
    }

    /// The param buffer `owner` binds: its own, else the shared zero one.
    pub(crate) fn params_buffer(&self, owner: Option<&str>) -> &wgpu::Buffer {
        owner
            .and_then(|o| self.params.get(o))
            .map_or(&self.zero_params, |(buffer, _)| buffer)
    }

    /// Make `owner`'s param buffer hold `values`: created on first use, written only
    /// when the values changed since the last write.
    pub(crate) fn write_params(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        owner: &str,
        values: PackedParams,
    ) {
        if let Some((buffer, last)) = self.params.get_mut(owner) {
            if *last != values {
                queue.write_buffer(buffer, 0, bytemuck::cast_slice(&values));
                *last = values;
            }
            return;
        }
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Material Shader Params"),
            size: std::mem::size_of::<PackedParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&buffer, 0, bytemuck::cast_slice(&values));
        self.params.insert(owner.to_owned(), (buffer, values));
    }

    /// Drop every group (their param buffers stay), so the next lookup rebuilds each
    /// over the textures resident now — called when a texture file was re-written
    /// (#689), before any draw of the frame resolves an index.
    pub(crate) fn forget_groups(&mut self) {
        self.index.clear();
        self.groups.clear();
    }

    /// Distinct material groups built so far.
    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.groups.len()
    }
}
