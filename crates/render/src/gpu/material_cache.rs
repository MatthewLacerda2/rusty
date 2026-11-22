use crate::gpu::ops::CountedQueue as _;
use std::collections::{BTreeSet, HashMap};

use rusty_scene::SceneId;
use rusty_shadergen::params::PackedParams;

pub(crate) const MATERIAL_MAPS: usize = 10;

pub(crate) const MATERIAL_TEXTURES: usize = MATERIAL_MAPS + rusty_shadergen::textures::SLOTS.len();

/// Albedo, metallic, roughness, normal, emissive, detail, occlusion, specular, height and detail
/// mask keys, then the extra shader texture slots (`mask`), in that order.
pub(crate) type MapSignature = [String; MATERIAL_TEXTURES];

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum ParamOwner {
    Material(SceneId, String),
    Entity(SceneId, u32, usize),
}

pub(crate) type GroupKey = (MapSignature, Option<ParamOwner>);

pub(crate) struct MaterialCache {
    index: HashMap<GroupKey, usize>,
    groups: Vec<Option<wgpu::BindGroup>>,
    free: Vec<usize>,
    zero_params: wgpu::Buffer,
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

    pub(crate) fn lookup(&self, key: &GroupKey) -> Option<usize> {
        self.index.get(key).copied()
    }

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

    /// Run before the frame resolves any index, so a freed slot is never one its batches name.
    pub(crate) fn release_entities(&mut self, scene: SceneId, overrides: impl Fn(u32) -> bool) {
        self.release(
            |o| matches!(o, ParamOwner::Entity(s, id, _) if *s == scene && !overrides(*id)),
        );
    }

    pub(crate) fn release_scene(&mut self, scene: SceneId) {
        self.release(|o| match o {
            ParamOwner::Material(s, _) | ParamOwner::Entity(s, ..) => *s == scene,
        });
    }

    fn release(&mut self, stale: impl Fn(&ParamOwner) -> bool) {
        self.params.retain(|owner, _| !stale(owner));
        let (groups, free) = (&mut self.groups, &mut self.free);
        self.index.retain(|(_, owner), i| {
            let keep = !owner.as_ref().is_some_and(&stale);
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

    pub(crate) fn write_params(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        owner: &ParamOwner,
        values: PackedParams,
    ) {
        if let Some((buffer, last)) = self.params.get_mut(owner) {
            if *last != values {
                queue.counted_write_buffer(buffer, 0, bytemuck::cast_slice(&values));
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
        queue.counted_write_buffer(&buffer, 0, bytemuck::cast_slice(&values));
        self.params.insert(owner.clone(), (buffer, values));
    }

    pub(crate) fn holds_params(&self, owner: &ParamOwner) -> bool {
        self.params.contains_key(owner)
    }

    /// Drops every group when a texture file is rewritten, before any draw resolves an index;
    /// param buffers stay.
    pub(crate) fn forget_groups(&mut self) {
        self.index.clear();
        self.groups.clear();
        self.free.clear();
    }

    /// The map keys the groups at `used` bind (never the empty default's).
    pub(crate) fn maps_of(&self, used: &BTreeSet<usize>) -> BTreeSet<String> {
        let keys = self.index.iter().filter(|(_, i)| used.contains(i));
        let maps = keys.flat_map(|((maps, _), _)| maps.iter());
        maps.filter(|m| !m.is_empty()).cloned().collect()
    }

    pub(crate) fn forget_groups_binding(&mut self, gone: impl Fn(&str) -> bool) {
        let (groups, free) = (&mut self.groups, &mut self.free);
        self.index.retain(|(maps, _), i| {
            let keep = !maps.iter().any(|m| gone(m));
            if !keep {
                groups[*i] = None;
                free.push(*i);
            }
            keep
        });
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.groups.iter().flatten().count()
    }

    #[cfg(test)]
    pub(crate) fn params_of(&self, owner: &ParamOwner) -> Option<&PackedParams> {
        self.params.get(owner).map(|(_, values)| values)
    }

    #[cfg(test)]
    pub(crate) fn params_len(&self) -> usize {
        self.params.len()
    }
}
