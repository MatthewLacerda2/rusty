//! Which lightmap each static mesh wears (#438): the scene document's references to
//! the baked lightmap files, keyed by entity id. Values and paths only — the
//! textures are PNG assets beside the scene, loaded by the renderer, never inlined
//! (the same split as a reflection probe's cubemap path).

use serde::{Deserialize, Serialize};

/// One lightmapped entity and its lightmap file.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LightmapEntry {
    pub entity: u32,
    /// The RGBM PNG (see `encode`), as the bake wrote it.
    pub path: String,
}

/// The scene's lightmaps. Saved in the scene document as a plain list; an entity
/// missing from it (dynamic, no lightmap UV, or added since the bake) keeps probe /
/// ambient lighting.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LightmapSet {
    entries: Vec<LightmapEntry>,
}

impl LightmapSet {
    /// The lightmap file `entity` wears, if any.
    pub fn get(&self, entity: u32) -> Option<&str> {
        self.entries
            .iter()
            .find(|e| e.entity == entity)
            .map(|e| e.path.as_str())
    }

    /// Point `entity` at `path`, replacing any lightmap it had.
    pub fn set(&mut self, entity: u32, path: String) {
        match self.entries.iter_mut().find(|e| e.entity == entity) {
            Some(e) => e.path = path,
            None => self.entries.push(LightmapEntry { entity, path }),
        }
    }

    pub fn entries(&self) -> &[LightmapEntry] {
        &self.entries
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Forget every lightmap: every mesh falls back to probe / ambient lighting.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_replaces_and_round_trips_as_a_plain_list() {
        let mut set = LightmapSet::default();
        set.set(3, "a.png".into());
        set.set(5, "b.png".into());
        set.set(3, "c.png".into());
        assert_eq!(
            (set.get(3), set.get(5), set.get(9)),
            (Some("c.png"), Some("b.png"), None)
        );
        let json = serde_json::to_string(&set).unwrap();
        assert!(json.starts_with('['), "{json}");
        assert_eq!(serde_json::from_str::<LightmapSet>(&json).unwrap(), set);
    }
}
