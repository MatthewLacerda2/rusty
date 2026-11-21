use glam::Vec3;

use crate::World;

#[derive(Default)]
pub(crate) struct BoundsQueue {
    pending: Vec<u32>,
    queued: Vec<bool>,
    seen: Vec<u32>,
    pass: u32,
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

    fn first_visit(&mut self, id: u32) -> bool {
        let at = id as usize;
        if at >= self.seen.len() {
            self.seen.resize(at + 1, 0);
        }
        std::mem::replace(&mut self.seen[at], self.pass) != self.pass
    }
}

impl World {
    pub(crate) fn moved(&mut self, id: u32) {
        self.world_space.moved(id);
        self.bounds.push(id);
        self.stamp_pose_below(id);
    }

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

    pub fn flush_bounds(&mut self) {
        if self.bounds.pending.is_empty() {
            return;
        }
        let mut pending = std::mem::take(&mut self.bounds.pending);
        let mut stack = std::mem::take(&mut self.bounds.stack);
        self.bounds.pass = self.bounds.pass.wrapping_add(1);
        if self.bounds.pass == 0 {
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

    pub fn refresh_collider_aabb(&mut self, id: u32) {
        if !self.has_collider(id) {
            return;
        }
        let Some(pose) = self.world_pose(id) else {
            return;
        };
        if let Some(mut col) = self.derived_mut::<rusty_components::ColliderComponent>(id) {
            (col.aabb_min, col.aabb_max) = col.calculate_world_aabb(pose);
        }
    }
}
