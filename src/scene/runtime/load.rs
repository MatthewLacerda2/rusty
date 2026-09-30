//! src/scene/runtime/load.rs — the scene half of runtime scene loading (#432).
//!
//! Play-mode `Scene.Load(path)` only records the request ([`Scene::request_load`]);
//! the tick's scene-load phase (after the destroy phase) takes it, runs the outgoing
//! scene's teardown callbacks, then calls [`Scene::swap_in`] — so the World is never
//! replaced mid-dispatch and a replay swaps on the same tick every time.
//!
//! One active scene stays the model. `Scene.DontDestroyOnLoad(id)` marks an entity
//! ([`Scene::dont_destroy_on_load`]); at the swap it and every descendant are carried
//! into the incoming World **with their ids**, because scripts, timers and the UI
//! event system hold those ids. An incoming entity whose id a survivor already holds
//! is moved to a fresh id instead (`make_room`), its references rewritten with it.

use std::collections::{BTreeMap, BTreeSet};

use crate::scene::serialize::{apply_scene_data, SceneData};
use crate::scene::{Entity, MaterialAsset, Scene};

impl Scene {
    /// Queue a play-mode load of `path`, swapped in at the tick's tail. A later
    /// request in the same tick replaces an earlier one: the last `Scene.Load` wins.
    pub fn request_load(&mut self, path: String) {
        self.pending_load = Some(path);
    }

    /// Take the queued load, if any — the scene-load phase's drain.
    pub fn take_pending_load(&mut self) -> Option<String> {
        self.pending_load.take()
    }

    /// Mark `id` to survive scene loads, with its descendants. `false` when no entity
    /// has that id. The mark lasts until the entity is destroyed or play stops.
    pub fn dont_destroy_on_load(&mut self, id: u32) -> bool {
        let exists = self.world.contains(id);
        if exists {
            self.persistent.insert(id);
        }
        exists
    }

    /// The entities a load carries across: every live marked entity and all of its
    /// descendants.
    pub fn load_survivors(&self) -> BTreeSet<u32> {
        let mut survivors = BTreeSet::new();
        let mut stack: Vec<u32> = self
            .persistent
            .iter()
            .copied()
            .filter(|&id| self.world.contains(id))
            .collect();
        while let Some(id) = stack.pop() {
            if survivors.insert(id) {
                stack.extend(self.world.children(id));
            }
        }
        survivors
    }

    /// Replace the World with `data` (the document read from `path`), carrying
    /// `survivors` across unchanged: same ids, same components, same order among
    /// themselves, appended after the incoming entities. A survivor whose parent does
    /// not survive becomes a root. The materials survivors reference come along
    /// unless the incoming library already defines that name. Decals and queued
    /// destroys of the outgoing scene are dropped, and the scene takes a fresh
    /// runtime identity — it is a different scene now.
    pub fn swap_in(
        &mut self,
        mut data: SceneData,
        path: &str,
        survivors: &BTreeSet<u32>,
    ) -> Result<(), String> {
        let carried: Vec<Entity> = self
            .entity_ids()
            .into_iter()
            .filter(|id| survivors.contains(id))
            .filter_map(|id| self.world.entity_document(id))
            .collect();
        let materials: BTreeMap<String, MaterialAsset> = carried
            .iter()
            .filter_map(|e| e.material.as_ref())
            .filter_map(|m| Some((m.material.clone(), self.materials.get(&m.material)?.clone())))
            .collect();

        make_room(&mut data, survivors);
        apply_scene_data(self, data);
        for mut entity in carried {
            if entity.parent_id.is_some_and(|p| !survivors.contains(&p)) {
                entity.parent_id = None;
            }
            self.world.insert_entity(entity);
        }
        for (name, material) in materials {
            self.materials.entry(name).or_insert(material);
        }
        self.persistent.retain(|id| survivors.contains(id));
        self.pending_destroy.retain(|id| survivors.contains(id));
        self.decals.clear();
        self.renew_id();
        crate::scene::lighting::io::load_lighting_sidecar(self, path)
    }
}

/// Move every incoming entity whose id is in `taken` to a fresh id past both the
/// document's ids and `taken`, in document order (so the result is a pure function of
/// the inputs), rewriting parent, child and component references to match.
pub(super) fn make_room(data: &mut SceneData, taken: &BTreeSet<u32>) {
    let mut next = data
        .entities
        .iter()
        .map(|e| e.id + 1)
        .chain(taken.iter().map(|&id| id + 1))
        .fold(data.next_entity_id, u32::max);
    let mut moved = BTreeMap::new();
    for entity in &data.entities {
        if taken.contains(&entity.id) {
            moved.insert(entity.id, next);
            next += 1;
        }
    }
    data.next_entity_id = next;
    if moved.is_empty() {
        return;
    }
    let remap = |id: u32| moved.get(&id).copied().unwrap_or(id);
    for entity in &mut data.entities {
        entity.id = remap(entity.id);
        entity.parent_id = entity.parent_id.map(remap);
        for child in &mut entity.children {
            *child = remap(*child);
        }
        entity.remap_refs(&|r| Some(remap(r)));
    }
    data.selected_entity_id = data.selected_entity_id.map(remap);
}
