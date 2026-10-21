//! src/ecs/world_space/bounds.rs — collider world AABBs, never read stale (#1157, #1170).
//!
//! A collider stores its world AABB (`aabb_min`/`aabb_max`). Two things change it:
//! - **its own collider** being set or reshaped, which refreshes it on the spot
//!   (`collider.rs`: [`World::set_collider`] and the [`ColliderMut`] guard);
//! - **a Transform write** on it or anything above it. That only queues the written
//!   entity ([`World::moved`], O(1)); [`World::flush_bounds`] later refreshes every
//!   collider under the queued entities, each once however many of its ancestors were
//!   written. The frame loop flushes after every stage, and so does every scene-level
//!   refresh (`Scene::update_entity_collider`, `Scene::update_all_colliders`).
//!
//! Between a write and the flush the stored box can lag, so a reader goes through
//! [`World::collider_bounds`]: the stored box when nothing above the collider is
//! queued, else the box worked out from its world pose there and then. A read is
//! never stale, mid-stage included.
//!
//! [`ColliderMut`]: super::ColliderMut

use glam::Vec3;

use crate::ecs::World;

/// The entities whose subtrees' collider AABBs are due a refresh.
#[derive(Default)]
pub(in crate::ecs) struct BoundsQueue {
    pending: Vec<u32>,
    queued: Vec<bool>,
    /// The flush pass that last refreshed each entity, so a flush visits it once.
    seen: Vec<u32>,
    pass: u32,
    /// The downward walk's stack, kept so a flush allocates nothing once warm.
    stack: Vec<u32>,
}

impl BoundsQueue {
    fn is_queued(&self, id: u32) -> bool {
        self.queued.get(id as usize).copied().unwrap_or(false)
    }

    fn push(&mut self, id: u32) {
        let at = id as usize;
        if at >= self.queued.len() {
            self.queued.resize(at + 1, false);
        }
        if !std::mem::replace(&mut self.queued[at], true) {
            self.pending.push(id);
        }
    }

    /// Whether `id` is first reached in this pass (and mark it reached).
    fn first_visit(&mut self, id: u32) -> bool {
        let at = id as usize;
        if at >= self.seen.len() {
            self.seen.resize(at + 1, 0);
        }
        std::mem::replace(&mut self.seen[at], self.pass) != self.pass
    }
}

impl World {
    /// `id`'s subtree moved in world space: its world matrices go dirty before the next
    /// read, and its colliders' AABBs are due a refresh. O(1).
    pub(in crate::ecs) fn moved(&mut self, id: u32) {
        self.world_space.moved(id);
        self.bounds.push(id);
    }

    /// `id`'s collider world AABB, `(min, max)`; `None` without a collider. Never stale:
    /// see the module note.
    pub fn collider_bounds(&self, id: u32) -> Option<(Vec3, Vec3)> {
        let col = self.collider(id)?;
        let queued = !self.bounds.pending.is_empty()
            && self
                .self_and_ancestors(id)
                .any(|a| self.bounds.is_queued(a));
        if !queued {
            return Some((col.aabb_min, col.aabb_max));
        }
        Some(col.calculate_world_aabb(self.world_pose(id)?))
    }

    /// Refresh the stored AABB of every collider under an entity moved since the last
    /// flush, each once, in the order the moves were queued.
    pub fn flush_bounds(&mut self) {
        if self.bounds.pending.is_empty() {
            return;
        }
        let mut pending = std::mem::take(&mut self.bounds.pending);
        let mut stack = std::mem::take(&mut self.bounds.stack);
        self.bounds.pass = self.bounds.pass.wrapping_add(1);
        if self.bounds.pass == 0 {
            // Wrapped: an old stamp could alias the new pass, so forget them all.
            self.bounds.seen.clear();
            self.bounds.pass = 1;
        }
        for &root in &pending {
            stack.push(root);
            while let Some(id) = stack.pop() {
                if self.bounds.first_visit(id) {
                    self.refresh_collider_aabb(id);
                    self.push_children(id, &mut stack);
                }
            }
        }
        for &root in &pending {
            self.bounds.queued[root as usize] = false;
        }
        pending.clear();
        (self.bounds.pending, self.bounds.stack) = (pending, stack);
    }

    /// Refresh `id`'s stored collider AABB from its world pose now (a no-op without a
    /// collider). Setting or reshaping a collider does this itself.
    pub fn refresh_collider_aabb(&mut self, id: u32) {
        let Some(pose) = self.world_pose(id) else {
            return;
        };
        if let Some(mut col) = self.component_mut::<crate::components::ColliderComponent>(id) {
            (col.aabb_min, col.aabb_max) = col.calculate_world_aabb(pose);
        }
    }
}
