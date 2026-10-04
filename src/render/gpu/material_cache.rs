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
//!
//! **Per-entity overrides (#670).** An entity overriding one of those params (Unity's
//! `MaterialPropertyBlock`) owns a buffer of its own, keyed by (scene, entity), holding
//! its material's values with its overrides on top — and so a group of its own, a draw
//! of its own. Entities without overrides keep sharing the material's group and
//! batching together. An entity's buffer and groups are released the frame after its
//! last override is cleared, and their slots reused.

use std::collections::HashMap;

use crate::scene::SceneId;
use crate::shadergen::params::PackedParams;

/// Group-2 textures: the five maps plus one per extra shader texture slot (#400).
pub(crate) const MATERIAL_TEXTURES: usize = 5 + crate::shadergen::textures::SLOTS.len();

/// Albedo, metallic, roughness, normal and emissive map keys, then the extra shader
/// texture slots (`mask`), in that order.
pub(crate) type MapSignature = [String; MATERIAL_TEXTURES];

/// Who owns a param buffer: a material (by library key), or one entity overriding
/// its material's values (#670).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum ParamOwner {
    Material(String),
    Entity(SceneId, u32),
}

/// A group's identity: its maps, and who owns its param buffer (if anyone).
pub(crate) type GroupKey = (MapSignature, Option<ParamOwner>);

pub(crate) struct MaterialCache {
    index: HashMap<GroupKey, usize>,
    /// Built groups by index; a released entity's slot is `None` until reused.
    groups: Vec<Option<wgpu::BindGroup>>,
    free: Vec<usize>,
    /// The params every material without runtime params binds (all zero).
    zero_params: wgpu::Buffer,
    /// Each owner's buffer, with the values last written to it.
    params: HashMap<ParamOwner, (wgpu::Buffer, PackedParams)>,
}

impl MaterialCache {
    pub(crate) fn new(zero_params: wgpu::Buffer) -> Self {
        Self {
            index: HashMap::new(),
            groups: Vec::new(),
            free: Vec::new(),
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
        let i = match self.free.pop() {
            Some(i) => i,
            None => {
                self.groups.push(None);
                self.groups.len() - 1
            }
        };
        self.groups[i] = Some(group);
        self.index.insert(key, i);
        i
    }

    pub(crate) fn group(&self, i: usize) -> &wgpu::BindGroup {
        self.groups[i]
            .as_ref()
            .expect("a released group is never drawn")
    }

    /// Release scene `scene`'s entity-owned buffers and groups whose entity no longer
    /// `overrides` (#670) — run before the frame resolves any index, so a freed slot
    /// is never one a batch of this frame points at.
    pub(crate) fn release_entities(&mut self, scene: SceneId, overrides: impl Fn(u32) -> bool) {
        let stale = |o: &ParamOwner| matches!(o, ParamOwner::Entity(s, id) if *s == scene && !overrides(*id));
        self.params.retain(|owner, _| !stale(owner));
        let (groups, free) = (&mut self.groups, &mut self.free);
        self.index.retain(|(_, owner), i| {
            let keep = !owner.as_ref().is_some_and(stale);
            if !keep {
                groups[*i] = None;
                free.push(*i);
            }
            keep
        });
    }

    /// The param buffer `owner` binds: its own, else the shared zero one.
    pub(crate) fn params_buffer(&self, owner: Option<&ParamOwner>) -> &wgpu::Buffer {
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
        owner: &ParamOwner,
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
        self.params.insert(owner.clone(), (buffer, values));
    }

    /// Drop every group (their param buffers stay), so the next lookup rebuilds each
    /// over the textures resident now — called when a texture file was re-written
    /// (#689), before any draw of the frame resolves an index.
    pub(crate) fn forget_groups(&mut self) {
        self.index.clear();
        self.groups.clear();
        self.free.clear();
    }

    /// Distinct material groups alive now.
    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.groups.iter().flatten().count()
    }

    /// Param buffers alive now (materials' and entities').
    #[cfg(test)]
    pub(crate) fn params_len(&self) -> usize {
        self.params.len()
    }
}
