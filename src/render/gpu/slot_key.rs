//! src/render/gpu/slot_key.rs — the per-scene key every pooled GPU resource lives
//! under (#355).
//!
//! Both persistent pools — the forward pass's entity/material bind groups
//! ([`entity_pool`](super::entity_pool)) and the shadow pass's model-matrix slots —
//! cache one resource per entity across frames. Keying that cache by the entity id
//! alone was the single-scene assumption in miniature: ids restart at 1 in every
//! `World`, so the editor scene's entity 1 and the Inspector preview's entity 1 hash
//! to the same slot. Two scenes in one frame then evicted and rebuilt each other's
//! resources every frame, re-introducing exactly the churn #210 removed.
//!
//! The key lives here, apart from either pool, because the *rule* — which slots a
//! prune may drop — has to be identical for both. One named predicate, two callers,
//! no drift.

use std::collections::HashSet;

use crate::scene::SceneId;

/// What a pooled slot belongs to: an entity *within a scene* (#355).
///
/// Entity ids restart at 1 in every fresh `World`, so the entity id alone is not a
/// key — two scenes rendered in the same frame would share slots and evict each
/// other's. Pairing it with the scene's identity makes the key globally unique.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct SlotKey {
    pub scene: SceneId,
    pub entity: u32,
}

impl SlotKey {
    pub(crate) fn new(scene: SceneId, entity: u32) -> Self {
        Self { scene, entity }
    }

    /// Whether a slot under this key survives a prune of `scene` against its `live`
    /// entity set.
    ///
    /// The rule both pools share, and the whole of the #355 per-scene fix: **another
    /// scene's slots always survive** — they were not part of this render, so their
    /// absence from `live` says nothing about them — while this scene's survive only
    /// while their entity is still live. Kept as one named predicate so the forward
    /// pool and the shadow pool cannot drift apart on it.
    pub(crate) fn survives_prune(&self, scene: SceneId, live: &HashSet<u32>) -> bool {
        self.scene != scene || live.contains(&self.entity)
    }
}

#[cfg(test)]
mod prune_tests {
    use super::*;
    use crate::scene::Scene;

    /// The prune rule, without a GPU: the pool's `retain` is a `HashMap::retain` over
    /// exactly this predicate, and the render-level guard test that exercises it end
    /// to end needs an adapter (it skips on machines without one). Pinning the rule
    /// here means the #355 invariant is checked on every machine, adapter or not.
    #[test]
    fn a_prune_of_one_scene_never_touches_another() {
        let (mine, theirs) = (Scene::new().id(), Scene::new().id());
        let live: HashSet<u32> = [1, 2].into_iter().collect();

        // My scene: live entities stay, dead ones go.
        assert!(SlotKey::new(mine, 1).survives_prune(mine, &live));
        assert!(!SlotKey::new(mine, 9).survives_prune(mine, &live));

        // Another scene's slots survive regardless — including one whose entity id
        // collides with a dead id of mine, which is the exact case that broke: every
        // `World` counts entity ids from 1, so collisions are the norm, not the edge.
        assert!(SlotKey::new(theirs, 9).survives_prune(mine, &live));
        assert!(SlotKey::new(theirs, 1).survives_prune(mine, &live));
    }

    #[test]
    fn the_key_distinguishes_the_same_entity_id_in_two_scenes() {
        let (a, b) = (Scene::new().id(), Scene::new().id());
        assert_ne!(SlotKey::new(a, 1), SlotKey::new(b, 1));
        assert_eq!(SlotKey::new(a, 1), SlotKey::new(a, 1));
    }
}
