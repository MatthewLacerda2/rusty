//! src/scene/destroy_queue.rs — the deferred-destroy command buffer (#323).
//!
//! Play-mode `Scene.DestroyEntity` doesn't remove an entity on the spot: it runs
//! *mid-dispatch* inside a script's callback, where the script runtime can't be
//! re-entered to fire the entity's teardown callbacks. So the verb QUEUES the id
//! here (like Unity's deferred `Object.Destroy`), and the script system drains
//! the queue at the tick's destroy phase — firing `OnDisable`/`OnDestroy` before
//! the real removal. The queue is a plain `Vec<u32>` field on `Scene`; these two
//! verbs are its only writers/readers, split out here to keep `scene.rs` lean.

use crate::scene::Scene;

impl Scene {
    /// Queue entity `id` for deferred destruction — the play-mode
    /// `Scene.DestroyEntity` path. The entity stays live until the script system
    /// drains the queue at the tick's destroy phase (firing `OnDisable`/`OnDestroy`
    /// before the real removal via [`Scene::destroy_entity`]).
    pub fn request_destroy(&mut self, id: u32) {
        self.pending_destroy.push(id);
    }

    /// Drain the deferred-destroy queue, sorted ascending and deduplicated: the
    /// destroy-dispatch order is deterministic, and an entity `DestroyEntity`-d
    /// twice in one tick is destroyed (and fires `OnDestroy`) exactly once.
    pub fn take_pending_destroys(&mut self) -> Vec<u32> {
        let mut ids = std::mem::take(&mut self.pending_destroy);
        ids.sort_unstable();
        ids.dedup();
        ids
    }
}

#[cfg(test)]
mod tests {
    use crate::scene::Scene;

    #[test]
    fn take_sorts_dedups_and_empties() {
        let mut scene = Scene::new();
        scene.request_destroy(7);
        scene.request_destroy(3);
        scene.request_destroy(7); // duplicate: one destroy only
        let ids = scene.take_pending_destroys();
        assert_eq!(ids, vec![3, 7], "ascending + deduped");
        assert!(
            scene.take_pending_destroys().is_empty(),
            "queue is drained after a take"
        );
    }
}
