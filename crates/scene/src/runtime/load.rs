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
//! moves too ([`super::renumber`]).

use std::collections::BTreeSet;

use crate::Scene;

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
}
