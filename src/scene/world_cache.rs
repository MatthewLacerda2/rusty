//! src/scene/world_cache.rs — the scene's world-space reads, over the ECS's store (#331, #1170).
//!
//! The world matrices used to live here, as a per-frame cache the render path filled
//! once and everything else bypassed with a live walk — two sources that could
//! disagree. They now live in `ecs::World`'s dirty-flagged store (`ecs::world_space`),
//! which a read cleans parents first, so one answer serves the render frame, the sim
//! mid-tick and edit mode alike. What stays is the scene's vocabulary for it and the
//! collider-AABB refreshes a caller asks for by hand (after changing a shape, or after
//! a load); a transform write refreshes its subtree's AABBs itself.

use glam::Mat4;

use crate::scene::Scene;

impl Scene {
    /// An entity's world matrix ([`World::local_to_world`]); identity for a dead id.
    ///
    /// [`World::local_to_world`]: crate::ecs::World::local_to_world
    pub fn compute_world_matrix(&self, entity_id: u32) -> Mat4 {
        self.world.local_to_world(entity_id)
    }

    /// The same matrix as [`Scene::compute_world_matrix`]: the render path's name for it.
    pub fn world_matrix(&self, entity_id: u32) -> Mat4 {
        self.world.local_to_world(entity_id)
    }

    /// Refresh one entity's collider world AABB from its world pose: what a changed
    /// collider shape needs. A moved transform refreshes its own.
    pub fn update_entity_collider(&mut self, id: u32) {
        self.world.refresh_collider_aabb(id);
    }

    /// Refresh the collider world-AABB of `id` and everything under it.
    pub fn update_subtree_colliders(&mut self, id: u32) {
        let ids: Vec<u32> = self.world.self_and_descendants(id).collect();
        for id in ids {
            self.world.refresh_collider_aabb(id);
        }
    }

    /// Refresh every entity's collider world-AABB (editor/load batch operation).
    pub fn update_all_colliders(&mut self) {
        let ids = self.world.ids().to_vec();
        for id in ids {
            self.world.refresh_collider_aabb(id);
        }
    }
}

#[cfg(test)]
#[path = "world_cache_guard_tests.rs"]
mod world_cache_guard_tests;
#[cfg(test)]
#[path = "world_cache_tests.rs"]
mod world_cache_tests;
