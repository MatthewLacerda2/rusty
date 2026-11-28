//! A floating-origin shift of the runtime navmesh: the cells keep their indices (the shift
//! is whole tiles), so only the grid's corner and what holds a world position move, and
//! nothing is rebaked.

use glam::Vec3;

use super::NavigationGraph;
use rusty_scene::Scene;

impl NavigationGraph {
    /// After the scene's shift; `sync_changes` before it, so the re-read bake inputs differ
    /// by the shift alone.
    pub fn shift_origin(&mut self, scene: &mut Scene, by: Vec3) {
        if !self.tracks(scene) {
            return;
        }
        self.translate(by);
        self.shift_baked(scene, by);
        self.requests.get_mut().shift_origin(by);
    }

    pub(crate) fn translate(&mut self, by: Vec3) {
        (self.min_x, self.max_x) = (self.min_x + by.x, self.max_x + by.x);
        // Never vertical (`FloatingOrigin::plan`): the spans' heights stay.
        (self.min_z, self.max_z) = (self.min_z + by.z, self.max_z + by.z);
        self.offmesh.shift_origin(by);
    }
}

#[cfg(test)]
#[path = "origin_tests.rs"]
mod origin_tests;
