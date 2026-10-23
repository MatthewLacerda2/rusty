//! src/scene/runtime/destroy_queue.rs — the deferred-destroy command buffer (#323).
//!
//! Play-mode `Scene.DestroyEntity` doesn't remove an entity on the spot: it runs
//! *mid-dispatch* inside a script's callback, where the script runtime can't be
//! re-entered to fire the entity's teardown callbacks. So the verb QUEUES the id
//! here (like Unity's deferred `Object.Destroy`), and the script system drains
//! the queue at the tick's destroy phase — firing `OnDisable`/`OnDestroy` before
//! the real removal. The queue is a plain `Vec<u32>` field on `Scene`; these
//! verbs are its only writers/readers, split out here to keep `scene.rs` lean.
//! Every engine destroy that may run in Play (`Scene.DestroyEntity`, a stale
//! hitbox) calls [`Scene::destroy`], which picks (#1298). An orphaned hitbox,
//! whose bone a re-import or a load dropped, is the one exception:
//! `Scene::attach_to_bones` removes it at once through `despawn_batch`, since it
//! runs outside Play or while a document is applied (`Despawn::Applying`).

use crate::scene::Scene;

impl Scene {
    /// Destroy `id` and its subtree the way the mode allows (#1298): in Play,
    /// queue it ([`Scene::request_destroy`]) so its scripts are torn down; in
    /// edit mode, where no script runs, remove it at once
    /// ([`Scene::destroy_entity`]).
    pub fn destroy(&mut self, id: u32) {
        if self.playing {
            self.request_destroy(id);
        } else {
            self.destroy_entity(id);
        }
    }

    /// Queue entity `id` for deferred destruction — the play-mode
    /// `Scene.DestroyEntity` path. The entity stays live until the script system
    /// drains the queue at the tick's destroy phase, which fires
    /// `OnDisable`/`OnDestroy` across its subtree and then despawns it
    /// (`ScriptManager::apply_pending_destroys`).
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

    /// Whether `id` goes at the next destroy phase: it, or an ancestor whose
    /// subtree the destroy takes, is queued (#1304).
    pub fn is_doomed(&self, id: u32) -> bool {
        std::iter::once(id)
            .chain(self.world.ancestors(id))
            .any(|a| self.pending_destroy.contains(&a))
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

    #[test]
    fn destroy_queues_in_play_and_removes_at_once_in_edit_mode() {
        let mut scene = Scene::new();
        let edit = scene.add_entity("Edit".into());
        scene.destroy(edit);
        assert!(!scene.world.contains(edit), "edit mode: gone at once");
        let play = scene.add_entity("Play".into());
        scene.set_playing(true);
        scene.destroy(play);
        assert!(scene.world.contains(play), "Play: live until the drain");
        assert_eq!(scene.take_pending_destroys(), vec![play]);
    }

    #[test]
    fn a_queued_ancestor_dooms_its_subtree() {
        let mut scene = Scene::new();
        let root = scene.add_entity("Root".into());
        let child = scene.add_entity("Child".into());
        let other = scene.add_entity("Other".into());
        scene.set_parent(child, Some(root)).unwrap();
        scene.request_destroy(root);
        assert!(scene.is_doomed(root) && scene.is_doomed(child));
        assert!(!scene.is_doomed(other));
    }

    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "skips script teardown")]
    fn an_immediate_destroy_in_play_is_refused() {
        let mut scene = Scene::new();
        let id = scene.add_entity("Doomed".into());
        scene.set_playing(true);
        scene.destroy_entity(id);
    }
}
