//! Which lightmap each static mesh wears (#438): the scene document's references to
//! the baked atlas pages, and per entity the page and the scale/offset into it
//! (Unity's `lightmapIndex` / `lightmapScaleOffset`). Paths and values only — the
//! pages are PNG assets beside the scene, loaded by the renderer, never inlined.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

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
    /// The atlas pages (RGBM PNGs, all the same size), in page order.
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
}

impl LightmapSet {
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
    }

    /// Forget every lightmap: every mesh falls back to probe / ambient lighting.
    pub fn clear(&mut self) {
        self.pages.clear();
        self.directions.clear();
        self.entries.clear();
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
        assert_eq!(set.get(5).map(|e| e.page), Some(1));
        assert!(set.get(9).is_none());
        let json = serde_json::to_string(&set).unwrap();
        assert_eq!(serde_json::from_str::<LightmapSet>(&json).unwrap(), set);
        assert!(json.find("\"entity\":3") < json.find("\"entity\":5"), "entity order");
        let twice = r#"{"entries":[{"entity":3,"page":0,"scale_offset":[0,0,0,0]},
            {"entity":3,"page":1,"scale_offset":[0,0,0,0]}]}"#;
        let first = serde_json::from_str::<LightmapSet>(twice).unwrap();
        assert_eq!(first.get(3).map(|e| e.page), Some(0), "the first one wins");
        assert_eq!(set.remove(3).map(|e| e.page), Some(0));
        assert!(set.get(3).is_none());
        set.clear();
        assert!(set.is_empty() && set.pages.is_empty() && set.directions.is_empty());
    }
}
