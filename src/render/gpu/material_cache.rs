//! src/render/gpu/material_cache.rs — one group-2 material bind group per distinct set
//! of resolved texture maps (#470).
//!
//! The per-entity pool (#210) built a material bind group per entity, so a hundred
//! crates held a hundred identical groups — and no two could share a draw call. Keyed
//! by the maps they bind instead, every crate resolves to the same index, which is
//! what lets [`BatchKey`](crate::render::draw::batch::BatchKey) group them.
//!
//! The key is the *resolved* signature (a map's path only once its texture is
//! resident, else empty for the default), so a late-loading texture gets a fresh group
//! rather than a stale one (#207). Entries are never evicted: there is one per material
//! look in use, a small set, and each is only a bind group over shared textures.

use std::collections::HashMap;

/// Albedo, metallic, roughness, normal and emissive map keys, in that order.
pub(crate) type MapSignature = [String; 5];

#[derive(Default)]
pub(crate) struct MaterialCache {
    index: HashMap<MapSignature, usize>,
    groups: Vec<wgpu::BindGroup>,
}

impl MaterialCache {
    /// The cache index for `sig`, if a group was already built for it.
    pub(crate) fn lookup(&self, sig: &MapSignature) -> Option<usize> {
        self.index.get(sig).copied()
    }

    /// Store `group` for `sig`, returning its index.
    pub(crate) fn insert(&mut self, sig: MapSignature, group: wgpu::BindGroup) -> usize {
        let i = self.groups.len();
        self.groups.push(group);
        self.index.insert(sig, i);
        i
    }

    pub(crate) fn group(&self, i: usize) -> &wgpu::BindGroup {
        &self.groups[i]
    }

    /// Distinct material groups built so far.
    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.groups.len()
    }
}
