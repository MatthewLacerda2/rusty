//! src/scene/world_cache/mod.rs — the scene's world-space reads, over the ECS's store (#331, #1170).
//!
//! The world matrices used to live here, as a per-frame cache the render path filled
//! once and everything else bypassed with a live walk — two sources that could
//! disagree. They now live in `ecs::World`'s dirty-flagged store (`ecs::world_space`),
//! which a read cleans parents first, so one answer serves the render frame, the sim
//! mid-tick and edit mode alike: read them there (`scene.world.local_to_world(id)`).
//! What stays here is the collider-AABB refreshes a caller asks for by hand (after a
//! load, which re-points parents wholesale). A transform write queues its subtree's AABBs, and setting or
//! reshaping a collider refreshes its own (`ecs::world_space::bounds`).

use crate::scene::Scene;

impl Scene {
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
mod guard_tests;
#[cfg(test)]
mod tests;
