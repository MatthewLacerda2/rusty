//! crates/scene/src/runtime/load.rs — the scene half of runtime scene loading (#432).
//!
//! Play-mode `Scene.Load(path)` only records the request ([`Scene::request_load`]);
//! the tick's scene-load phase (after the destroy phase) takes it, runs the outgoing
//! scene's teardown callbacks, then swaps the new scene in (`rusty_edit::load`'s
//! `swap_in`) — so the World is never replaced mid-dispatch and a replay swaps on the
//! same tick every time.
//!
//! One active scene stays the model. `Scene.DontDestroyOnLoad(id)` marks an entity
//! ([`Scene::dont_destroy_on_load`]); at the swap it and every descendant are carried
//! into the incoming World **with their ids**, because scripts, timers and the UI
//! event system hold those ids. An incoming entity whose id a survivor already holds
//! is moved to a fresh id instead (`make_room`), its references rewritten with it
//! through the one id-field walk; a survivor's references to the entities left
//! behind are cleared through the same walk. What the scene keys by entity id
//! outside the entities (the selection, the lightmaps, the baked PVS) follows the
//! moves through [`Scene::renumber`], whose [`RENUMBERED`] the state inventory holds
//! to every saved field keyed by entity (`state::renumber_tests`).

use std::collections::{BTreeMap, BTreeSet};

use crate::Scene;

/// A renumbering's moves, old id → new; every new id is fresh.
pub type Moves = BTreeMap<u32, u32>;

/// Every field of `Scene` outside the World that names entity ids loaded from the
/// scene's files, with how it follows a renumbering. [`Scene::renumber`] runs each
/// one, and the state inventory's test fails a saved field keyed by entity that is
/// not here.
pub const RENUMBERED: &[(&str, fn(&mut Scene, &Moves))] = &[
    ("selected_entity_id", |scene, moved| {
        let id = scene.selected_entity_id;
        scene.selected_entity_id = id.map(|id| moved.get(&id).copied().unwrap_or(id));
    }),
    ("lightmaps", |scene, moved| scene.lightmaps.remap(moved)),
    ("occlusion", renumber_occlusion),
];

/// The baked PVS was baked against the file's ids: rename the objects a load
/// moved, so a carried entity never turns the level's culling off.
fn renumber_occlusion(scene: &mut Scene, moved: &Moves) {
    let Some(data) = scene.occlusion.as_deref() else {
        return;
    };
    let remapped = data.remap(moved);
    if remapped.is_none() {
        log::warn!(
            "[Occlusion] the bake's ids collide after the load: nothing is occlusion-culled"
        );
    }
    *scene.occlusion = remapped.map(std::sync::Arc::new);
}

impl Scene {
    /// Queue a play-mode load of `path`, swapped in at the tick's tail. A later
    /// request replaces an earlier one: the last `Scene.Load` wins (`runtime::loads`).
    pub fn request_load(&mut self, path: String) {
        self.loads.request(path);
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
        self.persistent
            .iter()
            .flat_map(|&id| self.world.self_and_descendants(id))
            .collect()
    }

    /// Follow a load's renumbering (`moved`, old id → new) in every field keyed by
    /// entity outside the World ([`RENUMBERED`]), once the document and its
    /// sidecars are in: the World's own references were rewritten as its entities
    /// moved (`rusty_edit::load::make_room`). No moves, no change.
    pub fn renumber(&mut self, moved: &Moves) {
        if moved.is_empty() {
            return;
        }
        for (_, follow) in RENUMBERED {
            follow(self, moved);
        }
    }
}
