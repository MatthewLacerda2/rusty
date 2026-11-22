//! Play-mode `Scene.DestroyEntity` runs mid-dispatch, where the script runtime cannot be re-entered
//! for teardown, so it queues the id and the tick's destroy phase drains it.

use crate::Scene;

impl Scene {
    pub fn destroy(&mut self, id: u32) {
        // Recorded as removed from its model instance, so propagation never stamps it back.
        crate::prefab_link::removed::record_removed(self, id);
        if self.playing {
            self.request_destroy(id);
        } else {
            self.destroy_entity(id);
        }
    }

    pub fn request_destroy(&mut self, id: u32) {
        self.pending_destroy.push(id);
    }

    /// Sorted and deduplicated: the dispatch order is deterministic, and a double destroy fires
    /// `OnDestroy` once.
    pub fn take_pending_destroys(&mut self) -> Vec<u32> {
        let mut ids = std::mem::take(&mut self.pending_destroy);
        ids.sort_unstable();
        ids.dedup();
        ids
    }

    pub fn is_doomed(&self, id: u32) -> bool {
        std::iter::once(id)
            .chain(self.world.ancestors(id))
            .any(|a| self.pending_destroy.contains(&a))
    }
}

#[cfg(test)]
mod tests {
    use crate::Scene;

    #[test]
    fn take_sorts_dedups_and_empties() {
        let mut scene = Scene::new();
        scene.request_destroy(7);
        scene.request_destroy(3);
        scene.request_destroy(7);
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
