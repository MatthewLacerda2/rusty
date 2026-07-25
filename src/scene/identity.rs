//! src/scene/identity.rs — runtime identity for a live scene (#355).
//!
//! Entity ids are only unique *within* a `World`: every fresh one counts from 1, so
//! the editor scene's entity 3 and the Inspector preview's entity 3 are different
//! objects wearing the same number. The renderer caches per-entity GPU resources by
//! that number, which is fine while there is one scene and wrong the moment there
//! are two — the second scene's slots collide with the first's, and pruning against
//! one scene's live set evicts the other's.
//!
//! A [`SceneId`] is the missing half of that key. It identifies the live `Scene`
//! *instance*, not its contents: assigned once at construction, never serialized,
//! never reused. Save/load builds a new `Scene` and so earns a new id (its cached
//! GPU resources should not be inherited); entering and leaving Play keeps the same
//! `Scene` object, and so keeps its id (its resources should persist).
//!
//! The counter is a plain atomic — no wall clock, no RNG — so it respects the
//! determinism discipline even though the renderer that consumes it is platform
//! layer.

use std::sync::atomic::{AtomicU64, Ordering};

/// Source of the next identity. Starts at 1 so `SceneId(0)` can never be minted and
/// stays available as an "no scene" sentinel for anyone who needs one.
static NEXT: AtomicU64 = AtomicU64::new(1);

/// Identity of one live [`Scene`](crate::scene::Scene) instance. Cheap to copy and
/// hash — it is half of every per-entity cache key in the renderer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SceneId(u64);

impl SceneId {
    /// Mint the next identity. Called once per `Scene` construction; two scenes alive
    /// at the same time can never share one.
    pub fn next() -> Self {
        SceneId(NEXT.fetch_add(1, Ordering::Relaxed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::Scene;

    #[test]
    fn every_id_is_distinct() {
        let (a, b, c) = (SceneId::next(), SceneId::next(), SceneId::next());
        assert_ne!(a, b);
        assert_ne!(b, c);
        assert_ne!(a, c);
    }

    #[test]
    fn two_scenes_never_share_an_identity() {
        // The whole point: the editor scene and the Inspector preview's scene are
        // different objects even though their entity ids both start at 1.
        let first = Scene::new();
        let second = Scene::new();
        assert_ne!(first.id(), second.id());
        assert_eq!(first.id(), first.id(), "and an id is stable for its scene");
    }

    #[test]
    fn an_id_survives_the_play_mode_round_trip() {
        // Play snapshots to `SceneData` and applies it back to the SAME `Scene`, so
        // the identity — and therefore every cached GPU resource keyed by it — must
        // come through unchanged.
        let mut scene = Scene::new();
        let before = scene.id();
        let snapshot = crate::scene::snapshot::SceneSnapshot::capture(&scene);
        snapshot.restore(&mut scene);
        assert_eq!(scene.id(), before);
    }
}
