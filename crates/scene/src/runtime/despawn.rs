use std::collections::BTreeSet;

use crate::Scene;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Despawn {
    EditMode,
    TornDown,
    Applying,
}

impl Scene {
    pub fn is_playing(&self) -> bool {
        self.playing
    }

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
            self.persistent.clear();
            self.ragdolls = Default::default();
            self.debug_lines = Default::default();
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
        let decals = self.decals.take_owned(ids);
        self.keep_dropped(ids, decals);
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
            self.persistent.remove(id);
            self.lightmaps.remove(*id);
        }
        self.pending_destroy.retain(|e| !ids.contains(e));
    }
}
