use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use rusty_components::LightMode;

use crate::runtime::renumber::Moves;

/// The one rule `BakeScene::takes_mesh` and the renderer's `in_bake` read: such a mesh
/// occludes, bounces and emits in the bake, lightmapped or not.
pub fn bakes_mesh(scene: &crate::Scene, id: u32) -> bool {
    scene.world.is_active_in_hierarchy(id)
        && scene.world.is_static(id)
        && !scene.world.has_light(id)
        && scene.world.mesh(id).is_some_and(|m| !m.is_skinned())
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LightmapEntry {
    pub entity: u32,
    pub page: u32,
    /// Maps the mesh's lightmap UV into the page: `uv * xy + zw`.
    pub scale_offset: [f32; 4],
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct LightmapSet {
    #[serde(default)]
    pub pages: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub directions: Vec<String>,
    #[serde(default, with = "as_list")]
    entries: BTreeMap<u32, LightmapEntry>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub lights: BTreeSet<u32>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub baked_as: BTreeMap<u32, LightMode>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    /// Shadowmask pages, one per colour page: channel `c` is the share of `Mixed` channel
    /// `c` past static occluders, 1 lit.
    pub masks: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub channels: BTreeMap<u32, u8>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub fell_back: BTreeSet<u32>,
    #[serde(skip)]
    /// Under a preview, the scene's own lightmaps: what a save writes and a cancel restores.
    pub shown_over: Option<Box<LightmapSet>>,
}

impl LightmapSet {
    pub fn saved(&self) -> &LightmapSet {
        self.shown_over.as_deref().unwrap_or(self)
    }

    pub fn get(&self, entity: u32) -> Option<&LightmapEntry> {
        self.entries.get(&entity)
    }

    pub fn insert(&mut self, entry: LightmapEntry) {
        match &mut self.shown_over {
            Some(own) => own.insert(entry),
            None => drop(self.entries.insert(entry.entity, entry)),
        }
    }

    pub fn remove(&mut self, entity: u32) -> Option<LightmapEntry> {
        let shown = self.entries.remove(&entity);
        match &mut self.shown_over {
            Some(own) => own.remove(entity),
            None => shown,
        }
    }

    pub fn retain(&mut self, keep: impl Fn(u32) -> bool + Copy) {
        self.entries.retain(|&id, _| keep(id));
        if let Some(own) = &mut self.shown_over {
            own.retain(keep);
        }
    }

    /// `None` when two ids collide, as `OcclusionData::remap` refuses.
    pub fn remap(&self, moves: &Moves) -> Option<Self> {
        let to = |id: u32| moves.to(id);
        let entries: BTreeMap<_, _> = self
            .entries
            .values()
            .map(|e| {
                (
                    to(e.entity),
                    LightmapEntry {
                        entity: to(e.entity),
                        ..e.clone()
                    },
                )
            })
            .collect();
        let lights: BTreeSet<_> = self.lights.iter().map(|&id| to(id)).collect();
        let baked_as: BTreeMap<_, _> = self.baked_as.iter().map(|(&id, &m)| (to(id), m)).collect();
        let channels: BTreeMap<_, _> = self.channels.iter().map(|(&id, &c)| (to(id), c)).collect();
        let fell_back: BTreeSet<_> = self.fell_back.iter().map(|&id| to(id)).collect();
        let whole = entries.len() == self.entries.len()
            && lights.len() == self.lights.len()
            && baked_as.len() == self.baked_as.len()
            && channels.len() == self.channels.len()
            && fell_back.len() == self.fell_back.len();
        whole.then(|| Self {
            entries,
            lights,
            baked_as,
            channels,
            fell_back,
            ..self.clone()
        })
    }

    pub fn keep_only(&mut self, ids: &BTreeSet<u32>) {
        self.entries.retain(|id, _| ids.contains(id));
        self.lights.retain(|id| ids.contains(id));
        self.baked_as.retain(|id, _| ids.contains(id));
        self.channels.retain(|id, _| ids.contains(id));
        self.fell_back.retain(|id| ids.contains(id));
    }

    pub(crate) fn forget(&mut self, ids: &BTreeSet<u32>) {
        self.entries.retain(|id, _| !ids.contains(id));
        self.lights.retain(|id| !ids.contains(id));
        self.baked_as.retain(|id, _| !ids.contains(id));
        self.channels.retain(|id, _| !ids.contains(id));
        self.fell_back.retain(|id| !ids.contains(id));
    }

    pub fn entries(&self) -> impl Iterator<Item = &LightmapEntry> {
        self.entries.values()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
            && self.lights.is_empty()
            && self.baked_as.is_empty()
            && self.channels.is_empty()
            && self.fell_back.is_empty()
    }

    pub fn holds_bake(&self) -> bool {
        !self.entries.is_empty() || !self.lights.is_empty()
    }

    pub fn baked_light(&self, entity: u32) -> bool {
        self.lights.contains(&entity)
    }

    /// Reads `fell_back` through `baked_mode`: set that first.
    pub fn record_taken(&mut self, taken: &BTreeMap<u32, LightMode>) {
        let baked = taken.iter().map(|(&id, &m)| (id, self.baked_mode(id, m)));
        self.baked_as = baked.collect();
    }

    /// Test hook: mark `entity` taken in as `Baked`, as a bake would.
    #[doc(hidden)]
    pub fn took_in_baked(&mut self, entity: u32) {
        self.lights.insert(entity);
        self.baked_as.insert(entity, LightMode::Baked);
    }

    pub fn bakes_direct(&self, entity: u32) -> bool {
        self.baked_as.get(&entity) == Some(&LightMode::Baked)
    }

    pub fn mask_channel(&self, entity: u32) -> Option<u8> {
        self.channels.get(&entity).copied()
    }

    pub fn baked_mode(&self, entity: u32, mode: LightMode) -> LightMode {
        match mode == LightMode::Mixed && self.fell_back.contains(&entity) {
            true => LightMode::Baked,
            false => mode,
        }
    }

    pub fn clear(&mut self) {
        self.shown_over = None;
        self.pages.clear();
        self.directions.clear();
        self.entries.clear();
        self.lights.clear();
        self.baked_as.clear();
        self.masks.clear();
        self.channels.clear();
        self.fell_back.clear();
    }
}

impl Extend<LightmapEntry> for LightmapSet {
    fn extend<I: IntoIterator<Item = LightmapEntry>>(&mut self, entries: I) {
        entries.into_iter().for_each(|e| self.insert(e));
    }
}

mod as_list {
    use super::{BTreeMap, LightmapEntry};
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(
        entries: &BTreeMap<u32, LightmapEntry>,
        out: S,
    ) -> Result<S::Ok, S::Error> {
        out.collect_seq(entries.values())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        input: D,
    ) -> Result<BTreeMap<u32, LightmapEntry>, D::Error> {
        let mut entries = BTreeMap::new();
        for e in Vec::<LightmapEntry>::deserialize(input)? {
            entries.entry(e.entity).or_insert(e);
        }
        Ok(entries)
    }
}

#[cfg(test)]
#[path = "lightmap_set_tests.rs"]
mod tests;
