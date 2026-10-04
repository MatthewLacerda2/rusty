//! Which lightmap each static mesh wears (#438): the scene document's references to
//! the baked atlas pages, and per entity the page and the scale/offset into it
//! (Unity's `lightmapIndex` / `lightmapScaleOffset`). Paths and values only — the
//! pages are PNG assets beside the scene, loaded by the renderer, never inlined.

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
    #[serde(default)]
    pub entries: Vec<LightmapEntry>,
}

impl LightmapSet {
    /// Where `entity`'s lightmap is, if it has one.
    pub fn get(&self, entity: u32) -> Option<&LightmapEntry> {
        self.entries.iter().find(|e| e.entity == entity)
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Forget every lightmap: every mesh falls back to probe / ambient lighting.
    pub fn clear(&mut self) {
        self.pages.clear();
        self.entries.clear();
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
            entries: vec![entry(3, 0), entry(5, 1)],
        };
        assert_eq!(set.get(5).map(|e| e.page), Some(1));
        assert!(set.get(9).is_none());
        let json = serde_json::to_string(&set).unwrap();
        assert_eq!(serde_json::from_str::<LightmapSet>(&json).unwrap(), set);
        set.clear();
        assert!(set.is_empty() && set.pages.is_empty());
    }
}
