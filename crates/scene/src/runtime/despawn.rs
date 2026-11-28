//! In Play a removed entity's scripts are torn down first, so a despawn names why it may
//! skip that.

use std::collections::BTreeSet;

use crate::Scene;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Despawn {
    /// Outside Play, where no script runs; a debug assert refuses it in Play.
    EditMode,
    /// The destroy queue's drain: the subtree's teardown already fired.
    TornDown,
    /// A document being applied (a load, a restore, a prefab stamp): no script instance yet.
    Applying,
}

impl Scene {
    pub fn is_playing(&self) -> bool {
        self.playing
    }

    /// Entering Play forgets every `AIPerception` memory: call it before any `Start`, or a
    /// noise one reports is lost.
    pub fn set_playing(&mut self, playing: bool) {
        #[cfg(feature = "dev")]
        if playing != self.playing {
            self.drop_bake_request(if playing {
                "Play started"
            } else {
                "Play stopped"
            });
        }
        if playing && !self.playing {
            self.loads.enter_play();
            self.clear_undo();
            self.forget_perception();
        }
        if !playing && self.playing {
            self.loads = Default::default();
            self.pending_destroy.clear();
            self.loaded = Default::default();
            self.ragdolls = Default::default();
            self.debug_lines = Default::default();
            self.probes.set_origin(glam::Vec3::ZERO);
        }
        self.playing = playing;
    }

    fn forget_perception(&mut self) {
        for id in self.world.ids_with_ai_perception() {
            if self
                .world
                .ai_perception(id)
                .is_none_or(|c| c.memory.is_empty())
            {
                // A `_mut` access stamps a change, so one that knows nothing is left alone.
                continue;
            }
            if let Some(mut c) = self.world.ai_perception_mut(id) {
                c.memory = Default::default();
            }
        }
    }

    pub fn clear(&mut self) {
        #[cfg(feature = "dev")]
        self.drop_bake_request("a new scene was started");
        #[cfg(feature = "dev")]
        let status = std::mem::take(&mut self.lighting_bake_status);
        let (playing, limits) = (self.playing, self.decals.limits());
        *self = Scene::new();
        self.playing = playing;
        self.decals.set_limits(limits);
        #[cfg(feature = "dev")]
        {
            self.lighting_bake_status = status;
        }
    }

    pub fn destroy_entity(&mut self, id: u32) {
        let doomed = self.destroy_set(id);
        self.despawn_batch(&doomed, Despawn::EditMode);
    }

    pub fn destroy_set(&self, id: u32) -> BTreeSet<u32> {
        self.world.self_and_descendants(id).collect()
    }

    pub fn despawn_batch(&mut self, ids: &BTreeSet<u32>, why: Despawn) {
        debug_assert!(
            !(self.playing && why == Despawn::EditMode),
            "an edit-mode despawn of {ids:?} in Play skips script teardown; use Scene::destroy"
        );
        // Taken before the World forgets them, so Undo can bring them back.
        let decals = self.decals.take_owned(ids);
        self.keep_dropped(ids, decals);
        let orphans: Vec<(u32, Option<super::membership::Member>)> = ids
            .iter()
            .flat_map(|&id| self.world.children(id))
            .filter(|child| !ids.contains(child))
            .map(|child| (child, self.member_of(child)))
            .collect();
        self.world.despawn_all(ids);
        for &id in ids {
            self.shader_overrides.clear(id, None);
        }
        self.ragdolls.retain(|id| !ids.contains(&id));
        self.history_cuts.retain(|id| !ids.contains(&id));
        if self.selected_entity_id.is_some_and(|s| ids.contains(&s)) {
            self.selected_entity_id = None;
        }
        for id in ids {
            self.loaded.persistent.remove(id);
            self.loaded.forget(*id);
            self.lightmaps.remove(*id);
        }
        for (child, before) in orphans {
            self.keep_membership(child, before);
        }
        self.pending_destroy.retain(|e| !ids.contains(e));
    }
}
