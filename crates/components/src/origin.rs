//! A floating-origin shift of the runtime state components keep in world space: every
//! position moves by the same `by`, so nothing reads the shift as motion.

use glam::{Mat4, Vec3};

use crate::particle::{EmitterFrame, SimulationSpace};
use crate::{
    AIPerceptionComponent, LineComponent, Memory, NavMeshAgentComponent, NavMeshObstacleComponent,
    ParticleEmitterComponent, TrailComponent,
};

impl NavMeshAgentComponent {
    pub fn shift_origin(&mut self, by: Vec3) {
        self.target += by;
        self.planned_target += by;
        for p in &mut self.cached_path {
            *p += by;
        }
        if let Some(at) = self.replan_from.as_mut() {
            *at += by;
        }
        for (_, link) in &mut self.path_links {
            (link.start, link.end) = (link.start + by, link.end + by);
        }
        if let Some(link) = self.off_mesh_link.as_mut() {
            (link.start, link.end) = (link.start + by, link.end + by);
        }
    }
}

impl NavMeshObstacleComponent {
    pub fn shift_origin(&mut self, by: Vec3) {
        if let Some(at) = self.last_position.as_mut() {
            *at += by;
        }
    }
}

impl TrailComponent {
    pub fn shift_origin(&mut self, by: Vec3) {
        for p in &mut self.runtime.points {
            p.position += by;
        }
    }
}

impl LineComponent {
    /// Local-space points move with their entity already.
    pub fn shift_origin(&mut self, by: Vec3) {
        if self.use_world_space {
            for p in &mut self.positions {
                *p += by;
            }
        }
    }
}

impl Memory {
    pub fn shift_origin(&mut self, by: Vec3) {
        for known in self.known.values_mut() {
            known.position += by;
        }
        for update in &mut self.updates {
            update.info.position += by;
        }
    }
}

impl AIPerceptionComponent {
    pub fn shift_origin(&mut self, by: Vec3) {
        self.memory.shift_origin(by);
    }
}

fn moved(frame: &mut EmitterFrame, by: Vec3) {
    frame.matrix = Mat4::from_translation(by) * frame.matrix;
}

impl ParticleEmitterComponent {
    /// `anchor` moves too, so a `Local` emitter's next carry sees no jump; `base` stays, so
    /// its draw space (`anchor` over `base`) carries its GPU births by the shift.
    pub fn shift_origin(&mut self, by: Vec3) {
        let runtime = &mut self.runtime;
        if let Some(frame) = runtime.anchor.as_mut() {
            moved(frame, by);
        }
        for p in &mut runtime.particles {
            p.position += by;
        }
        for e in &mut runtime.pending {
            e.position += by;
        }
        if self.simulation_space == SimulationSpace::World {
            runtime.gpu.shift(by);
        }
        runtime.revision = runtime.revision.wrapping_add(1);
    }
}

#[cfg(test)]
#[path = "origin_tests.rs"]
mod origin_tests;
