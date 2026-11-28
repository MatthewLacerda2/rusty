//! Unity's multi-scene editing: a root moved between loaded scenes, an empty one added.

use std::collections::BTreeSet;

use super::file::SceneFile;
use super::{Loaded, Member};
use crate::Scene;

impl Scene {
    /// Unity's `MoveGameObjectToScene`, for a root; no Undo step, as steps name no scene.
    pub fn move_to_scene(&mut self, id: u32, handle: u32) -> Result<(), String> {
        if !self.world.contains(id) {
            return Err("no live entity has that id".to_string());
        }
        if self.world.parent_id(id).is_some() {
            return Err("only a root moves to another scene; its children follow it".to_string());
        }
        if self.loaded.file(handle).is_none() {
            return Err("that scene is not loaded".to_string());
        }
        self.loaded.persistent.remove(&id);
        self.loaded.roots.insert(id, handle);
        if !self.playing {
            self.mark_unsaved();
        }
        Ok(())
    }

    /// `carried` is an empty document; the World's layers go in it.
    pub fn add_empty_scene(
        &mut self,
        base: Option<String>,
        path: String,
        mut carried: crate::serialize::SceneData,
    ) -> u32 {
        self.open_loaded(base);
        let fresh = Scene::new();
        let mut file = SceneFile::new(Default::default(), 1, 1);
        carried.layers = self.layers.clone();
        carried.collision_matrix = self.collision_matrix.clone();
        file.carried = Some(Box::new(carried));
        let loaded = &mut self.loaded;
        let handle = loaded.next;
        loaded.next += 1;
        loaded.scenes.push(Loaded {
            handle,
            path: Some(path),
            settings: Some(fresh.settings()),
            file,
        });
        handle
    }

    pub fn members(&self, handle: Option<u32>) -> Vec<u32> {
        let ids = self.world.ids().iter().copied();
        let wanted = Some(Member::Scene(handle));
        ids.filter(|&id| self.member_of(id) == wanted).collect()
    }

    /// No saved file can hold that link. A new entity, one no loaded file holds yet (a prefab
    /// stamped, a bone rebuilt), is no move: it takes its parent's scene, as Unity's
    /// `Instantiate(original, parent)` does.
    pub(crate) fn parent_crosses_scenes(&self, id: u32, parent: Option<u32>) -> bool {
        let Some(parent) = parent else {
            return false;
        };
        let several = !self.loaded.scenes.is_empty();
        several && !self.playing && self.member_of(id) != self.member_of(parent) && !self.is_new(id)
    }

    fn is_new(&self, id: u32) -> bool {
        let persistent = self.loaded.persistent.contains(&id);
        !persistent && !self.loaded.scenes.iter().any(|s| s.file.holds(id))
    }

    pub(super) fn unload_now(&mut self, members: &[u32]) {
        let doomed: BTreeSet<u32> = members
            .iter()
            .flat_map(|&id| self.destroy_set(id))
            .collect();
        self.despawn_batch(&doomed, crate::Despawn::EditMode);
        self.clear_undo();
    }
}
