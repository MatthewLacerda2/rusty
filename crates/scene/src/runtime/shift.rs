//! A floating-origin shift of the scene: every root moves by a whole number of
//! `ORIGIN_STEP`s (a power of two, so a far coordinate loses no bit), and with it each
//! store the scene keeps in world space. A shift is a pose cut (`World::cut_pose`): the
//! renderer draws it where it lands, physics warps its bodies, and no velocity changes.
//! Baked data in authored space (probes, reflection boxes, the PVS) is not rewritten;
//! its readers add [`FloatingOrigin::offset`](super::origin::FloatingOrigin::offset).

use glam::{IVec3, Vec3};

use crate::Scene;

impl Scene {
    /// Shifts the World by `steps`; returns what every position moved by.
    pub fn shift_origin(&mut self, steps: IVec3) -> Vec3 {
        let by = self.floating_origin.shift(steps);
        let roots: Vec<u32> = (self.world.ids().iter().copied())
            .filter(|&id| self.world.parent_id(id).is_none())
            .collect();
        for &id in &roots {
            if let Some(mut t) = self.world.transform_mut(id) {
                t.position += by;
            }
            self.world.cut_pose(id);
        }
        self.shift_components(by);
        self.decals.shift_origin(by);
        self.debug_lines.shift_origin(by);
        by
    }

    /// Roots that joined in authored space (an `"Additive"` load) move into the World's.
    pub fn join_shifted(&mut self, roots: &[u32]) {
        if self.floating_origin.is_zero() {
            return;
        }
        let by = -self.floating_origin.offset_f32();
        for &id in roots {
            if let Some(mut t) = self.world.transform_mut(id) {
                t.position += by;
            }
            self.world.cut_pose(id);
        }
    }

    fn shift_components(&mut self, by: Vec3) {
        let w = &mut self.world;
        macro_rules! each {
            ($ids:ident, $get:ident, $get_mut:ident $(, $when:expr)?) => {
                for id in w.$ids() {
                    $(if !w.$get(id).is_some_and($when) { continue; })?
                    if let Some(mut c) = w.$get_mut(id) {
                        c.shift_origin(by);
                    }
                }
            };
        }
        each!(ids_with_nav_agent, nav_agent, nav_agent_mut);
        each!(ids_with_nav_obstacle, nav_obstacle, nav_obstacle_mut, |o| o
            .last_position
            .is_some());
        each!(ids_with_trail, trail, trail_mut, |t| !t
            .runtime
            .points
            .is_empty());
        each!(ids_with_line, line, line_mut, |l| l.use_world_space);
        each!(ids_with_particles, particles, particles_mut);
        each!(
            ids_with_ai_perception,
            ai_perception,
            ai_perception_mut,
            |p| !p.memory.is_empty()
        );
    }
}

#[cfg(test)]
#[path = "shift_tests.rs"]
mod shift_tests;
