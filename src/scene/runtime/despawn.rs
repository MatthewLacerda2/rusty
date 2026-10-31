//! The one door every despawn goes through (#1264), and the play state that
//! guards it (#1298, #1302).
//!
//! In Play a removed entity's scripts must get `OnDisable`/`OnDestroy` first, so
//! a despawn names why it may skip that ([`Despawn`]). Only three reasons hold:
//! edit mode, where no script runs; a teardown that already fired (the destroy
//! queue's drain); a document being applied, whose entities have no script
//! instance yet. Claiming edit mode in Play trips a debug assert, so an immediate
//! destroy that would leak a script instance fails loudly in tests.

use std::collections::BTreeSet;

use crate::scene::Scene;

/// Why a despawn may skip script teardown (`Scene::despawn_batch`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Despawn {
    /// Outside Play, where no script runs. Refused in Play (debug builds).
    EditMode,
    /// The destroy queue's drain: the subtree's teardown already fired.
    TornDown,
    /// A document being applied (a load, a snapshot restore, a prefab stamp):
    /// what it removes was made by the same document and has no script instance.
    Applying,
}

impl Scene {
    /// Whether this World is in Play: the single play state (#1302).
    pub fn is_playing(&self) -> bool {
        self.playing
    }

    /// Enter or leave Play. Only `GameWorld`'s transition (`enter_play` /
    /// `exit_play`) calls this outside tests, so every reader sees the state
    /// change at the same moment.
    /// Entering Play forgets the Undo history: Stop restores Play's changes.
    pub(crate) fn set_playing(&mut self, playing: bool) {
        if playing && !self.playing {
            self.clear_undo();
        }
        self.playing = playing;
    }

    /// Replace this scene with an empty one in place, keeping its play state and
    /// its decal budgets (File ▸ New, Reset Scene): the scene is new, the session
    /// (and the project's budgets it applied) is not.
    pub fn clear(&mut self) {
        let (playing, limits) = (self.playing, self.decals.limits());
        *self = Scene::new();
        self.playing = playing;
        self.decals.set_limits(limits);
    }

    /// Remove entity `id` and everything under it ([`Scene::destroy_set`]), as
    /// Unity's `Object.Destroy` takes a GameObject's children (#1283). Edit mode
    /// only: in Play a destroy must tear its scripts down first, so it goes
    /// through [`Scene::destroy`] (#1298).
    pub fn destroy_entity(&mut self, id: u32) {
        let doomed = self.destroy_set(id);
        self.despawn_batch(&doomed, Despawn::EditMode);
    }

    /// What destroying `id` removes: `id` and every descendant, so a skinned
    /// entity's bones and what hangs from them, and a bone's subtree (#1273). The
    /// one answer [`Scene::destroy_entity`] despawns and the script teardown
    /// notifies, so the two cannot drift (#1283). Empty for a dead id.
    pub fn destroy_set(&self, id: u32) -> BTreeSet<u32> {
        self.world.self_and_descendants(id).collect()
    }

    /// Despawn every entity of `ids` together, then drop the scene's own references
    /// to them (#1264): the selection, `DontDestroyOnLoad`, a queued destroy, their
    /// lightmap entries and shader overrides. The World's references go in
    /// `World::despawn_all`, one pass for the batch. Every despawn in the scene runs
    /// through here, saying `why` it may skip script teardown.
    pub(crate) fn despawn_batch(&mut self, ids: &BTreeSet<u32>, why: Despawn) {
        debug_assert!(
            !(self.playing && why == Despawn::EditMode),
            "an edit-mode despawn of {ids:?} in Play skips script teardown; use Scene::destroy"
        );
        self.world.despawn_all(ids);
        for &id in ids {
            self.shader_overrides.clear(id, None);
        }
        if self.selected_entity_id.is_some_and(|s| ids.contains(&s)) {
            self.selected_entity_id = None;
        }
        self.persistent.retain(|id| !ids.contains(id));
        self.pending_destroy.retain(|e| !ids.contains(e));
        self.lightmaps.entries.retain(|e| !ids.contains(&e.entity));
    }
}
