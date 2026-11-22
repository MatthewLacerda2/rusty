//! A live `Scene`'s identity, the other half of the renderer's per-entity cache keys: entity
//! ids count from 1 in every World. A load mints a new one; Play and Stop keep it.

use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SceneId(u64);

impl SceneId {
    pub fn next() -> Self {
        SceneId(NEXT.fetch_add(1, Ordering::Relaxed))
    }
}

impl crate::Scene {
    pub fn id(&self) -> SceneId {
        self.id
    }

    pub fn renew_id(&mut self) {
        self.id = SceneId::next();
    }

    /// Stop's restore after a play-mode `Scene.Load`: the restored World is that scene again, so
    /// a Generate Lighting waiting for Stop still recognises it.
    pub fn restore_id(&mut self, id: SceneId) {
        self.id = id;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Scene;

    #[test]
    fn every_id_is_distinct() {
        let (a, b, c) = (SceneId::next(), SceneId::next(), SceneId::next());
        assert_ne!(a, b);
        assert_ne!(b, c);
        assert_ne!(a, c);
    }

    #[test]
    fn two_scenes_never_share_an_identity() {
        let first = Scene::new();
        let second = Scene::new();
        assert_ne!(first.id(), second.id());
        assert_eq!(first.id(), first.id(), "and an id is stable for its scene");
    }
}
