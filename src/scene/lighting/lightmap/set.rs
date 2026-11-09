//! Which lightmap each static mesh wears (#438): the scene document's references to
//! the baked atlas pages, and per entity the page and the scale/offset into it
//! (Unity's `lightmapIndex` / `lightmapScaleOffset`). Paths and values only — the
//! pages are KTX2 files beside the scene (BC6H with mips, #1694; an RGBM PNG of the
//! same name beside each), loaded by the renderer, never inlined.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::components::LightMode;

/// One lightmapped entity: which page, and where in it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LightmapEntry {
    pub entity: u32,
    /// Index into [`LightmapSet::pages`].
    pub page: u32,
    /// Maps the mesh's lightmap UV into the page: `uv * xy + zw`.
    pub scale_offset: [f32; 4],
}

/// The scene's lightmaps. An entity missing from it (dynamic, no lightmap UV, or
/// added since the bake) keeps probe / ambient lighting.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct LightmapSet {
    /// The atlas pages, all the same size, in page order: `.ktx2` (#1694), or the
    /// `.png` of a scene baked before it.
    #[serde(default)]
    pub pages: Vec<String>,
    /// The direction pages of a directional bake (#810), one per colour page and
    /// packed the same, so an entry's page and scale/offset address both. Empty when
    /// the bake was not directional: the lightmap then ignores normal maps.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub directions: Vec<String>,
    /// Keyed by entity, so a draw's lookup and a despawn's removal are O(log n)
    /// (#1708); saved as a list in entity order.
    #[serde(default, with = "as_list")]
    entries: BTreeMap<u32, LightmapEntry>,
    /// The lights this bake took in, by entity (#1750, Unity's
    /// `Light.bakingOutput`): the `Mixed` and `Baked` lights active when it ran,
    /// whose bounce the pages hold, so SSIL gathers none of their light. A light
    /// moved, retuned or destroyed since still reads as baked, as in Unity, until
    /// the next bake; one spawned since (a script's muzzle flash) does not. Entity
    /// ids are never handed out twice in a session (`ecs::World::spawn`), but an
    /// Undo that takes a light's creation back gives its id back.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub lights: BTreeSet<u32>,
    /// The shadowmask pages of a `Shadowmask` bake (#1695), one per colour page and
    /// packed the same: RGBA8 PNGs whose channel `c` holds, per texel, how much of
    /// the `Mixed` light on channel `c` reaches it past static occluders (1 lit,
    /// 0 shadowed). Empty for a `BakedIndirect` bake.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub masks: Vec<String>,
    /// Each shadowmasked light's channel, by entity (#1695, Unity's
    /// `Light.bakingOutput.occlusionMaskChannel`): the mask pages' and the light
    /// probes' channel holding its static occlusion. A light the bake gave none
    /// to is not here.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub channels: BTreeMap<u32, u8>,
    /// The `Mixed` lights a `Shadowmask` bake found no free channel for (#1695):
    /// more than four overlapped. As in Unity, each baked as `Baked` (its direct
    /// light is in the lightmaps and the probes), and the frame draws it no more
    /// until a bake gives it a channel.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub fell_back: BTreeSet<u32>,
    /// While a progressive bake's preview is shown (#1970), the lightmaps the
    /// scene had before the bake: what a save writes ([`Self::saved`]) and what
    /// a cancel puts back. Never serialized.
    #[serde(skip)]
    pub shown_over: Option<Box<LightmapSet>>,
}

impl LightmapSet {
    /// The lightmaps a save writes: these, or while a preview is shown the ones it
    /// is shown over ([`Self::shown_over`]), so a preview's pages never reach a file.
    pub fn saved(&self) -> &LightmapSet {
        self.shown_over.as_deref().unwrap_or(self)
    }

    /// Where `entity`'s lightmap is, if it has one.
    pub fn get(&self, entity: u32) -> Option<&LightmapEntry> {
        self.entries.get(&entity)
    }

    /// Give `entry.entity` this lightmap, replacing the one it had.
    pub fn insert(&mut self, entry: LightmapEntry) {
        self.entries.insert(entry.entity, entry);
    }

    /// Take `entity`'s lightmap away (a despawn), returning it.
    pub fn remove(&mut self, entity: u32) -> Option<LightmapEntry> {
        self.entries.remove(&entity)
    }

    /// Every entry, in entity order.
    pub fn entries(&self) -> impl Iterator<Item = &LightmapEntry> {
        self.entries.values()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
            && self.lights.is_empty()
            && self.channels.is_empty()
            && self.fell_back.is_empty()
    }

    /// Whether a lightmap bake's light is held: a lightmap or a light it took in.
    /// A `Shadowmask` bake with nothing to lightmap holds only its channels (#1695),
    /// whose light is nowhere in a page.
    pub fn holds_bake(&self) -> bool {
        !self.entries.is_empty() || !self.lights.is_empty()
    }

    /// Whether the bake took light `entity` in ([`Self::lights`]).
    pub fn baked_light(&self, entity: u32) -> bool {
        self.lights.contains(&entity)
    }

    /// The shadowmask channel the bake gave light `entity` ([`Self::channels`]).
    pub fn mask_channel(&self, entity: u32) -> Option<u8> {
        self.channels.get(&entity).copied()
    }

    /// The mode light `entity` was baked as: `mode`, but `Baked` for a `Mixed`
    /// light that fell back for want of a shadowmask channel ([`Self::fell_back`]).
    /// What the frame draws it as until the next bake; a fallen-back light set to
    /// another mode since draws as that one.
    pub fn baked_mode(&self, entity: u32, mode: LightMode) -> LightMode {
        match mode == LightMode::Mixed && self.fell_back.contains(&entity) {
            true => LightMode::Baked,
            false => mode,
        }
    }

    /// Forget every lightmap: every mesh falls back to probe / ambient lighting.
    pub fn clear(&mut self) {
        self.pages.clear();
        self.directions.clear();
        self.entries.clear();
        self.lights.clear();
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

/// The entries as the file writes them: a list, each naming its entity. A file
/// naming one entity twice keeps the first, as the list lookup it replaced did.
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
mod tests {
    use super::*;

    #[test]
    fn lookup_clear_and_round_trip() {
        let entry = |entity, page| LightmapEntry {
            entity,
            page,
            scale_offset: [0.5, 0.5, 0.0, 0.5],
        };
        let mut set = LightmapSet {
            pages: vec!["a.png".into(), "b.png".into()],
            directions: vec!["a_dir.png".into(), "b_dir.png".into()],
            ..LightmapSet::default()
        };
        set.extend([entry(5, 1), entry(3, 0)]);
        set.lights.insert(7);
        assert!(set.baked_light(7) && !set.baked_light(5));
        assert_eq!(set.get(5).map(|e| e.page), Some(1));
        assert!(set.get(9).is_none());
        let json = serde_json::to_string(&set).unwrap();
        assert_eq!(serde_json::from_str::<LightmapSet>(&json).unwrap(), set);
        assert!(
            json.find("\"entity\":3") < json.find("\"entity\":5"),
            "entity order"
        );
        let twice = r#"{"entries":[{"entity":3,"page":0,"scale_offset":[0,0,0,0]},
            {"entity":3,"page":1,"scale_offset":[0,0,0,0]}]}"#;
        let first = serde_json::from_str::<LightmapSet>(twice).unwrap();
        assert_eq!(first.get(3).map(|e| e.page), Some(0), "the first one wins");
        assert_eq!(set.remove(3).map(|e| e.page), Some(0));
        assert!(set.get(3).is_none());
        set.clear();
        assert!(set.is_empty() && set.pages.is_empty() && set.directions.is_empty());
        assert!(!set.baked_light(7), "a clear forgets the lights too");
    }
}
