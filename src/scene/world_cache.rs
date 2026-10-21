//! src/scene/world_cache.rs — the scene's world-space reads, over the ECS's store (#331, #1170).
//!
//! The world matrices used to live here, as a per-frame cache the render path filled
//! once and everything else bypassed with a live walk — two sources that could
//! disagree. They now live in `ecs::World`'s dirty-flagged store (`ecs::world_space`),
//! which a read cleans parents first, so one answer serves the render frame, the sim
//! mid-tick and edit mode alike. What stays is the scene's vocabulary for it and the
//! collider-AABB refreshes a caller asks for by hand (after a load, which re-points
//! parents wholesale). A transform write queues its subtree's AABBs, and setting or
//! reshaping a collider refreshes its own (`ecs::world_space::bounds`).

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

    /// Bring one entity's stored collider AABB up to date, along with every one a
    /// transform write has queued ([`World::flush_bounds`]).
    ///
    /// [`World::flush_bounds`]: crate::ecs::World::flush_bounds
    pub fn update_entity_collider(&mut self, id: u32) {
        self.world.flush_bounds();
        self.world.refresh_collider_aabb(id);
    }

    /// Refresh every entity's collider world-AABB (editor/load batch operation).
    pub fn update_all_colliders(&mut self) {
        self.world.flush_bounds();
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
