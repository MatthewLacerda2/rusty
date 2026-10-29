//! src/components/particle/space.rs — Unity's **Simulation Space** (#1529).
//!
//! Particles are always stored in world space, so the draw pass, soft particles,
//! lighting, sorting, collisions and sub-emitters read one kind of position. A
//! `Local` emitter keeps them relative to itself by *carrying* them: the runtime
//! remembers the emitter's world matrix the live particles were last placed under
//! (its anchor), and [`ParticleEmitterComponent::carry_to`] moves every live
//! particle by the change from that matrix to the emitter's current one —
//! position, velocity and mesh tumble axis, so they move, turn and scale with the
//! emitter and any parent bone. Every spawn carries first and then places the new
//! particles under the same current matrix, so a particle born between ticks (a
//! scripted `Burst` after the gun moved) is never carried by a move that happened
//! before its birth.
//!
//! Unity's third option, `Custom` (relative to another entity), is left out: the
//! scene format has no field kind that references another entity by value yet.

use glam::{Mat4, Vec3};

use super::ParticleEmitterComponent;

crate::core::named_enum! {
    /// Which space live particles are simulated in.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum SimulationSpace {
        /// Particles stay where they were born; the emitter moving leaves them
        /// behind (smoke, debris).
        #[default]
        World = "World",
        /// Particles move, turn and scale with the emitter (a muzzle flash, a jet,
        /// an exhaust on a moving object).
        Local = "Local",
    }
}

/// A matrix this close to singular (a zero scale on the emitter or above it) has
/// no usable inverse: the carry from it is skipped.
const SINGULAR: f32 = 1e-12;

impl ParticleEmitterComponent {
    /// Bring a `Local` emitter's live particles to the emitter's world matrix
    /// `frame`, by the change from the matrix they were last placed under; then
    /// remember `frame`. A no-op for a `World` emitter. Idempotent for one `frame`.
    pub fn carry_to(&mut self, frame: Mat4) {
        if self.simulation_space != SimulationSpace::Local {
            return;
        }
        let runtime = &mut self.runtime;
        if let Some(anchor) = runtime.anchor.filter(|a| *a != frame) {
            if anchor.determinant().abs() > SINGULAR {
                let delta = frame * anchor.inverse();
                for p in &mut runtime.particles {
                    p.position = delta.transform_point3(p.position);
                    p.velocity = delta.transform_vector3(p.velocity);
                    p.axis = delta.transform_vector3(p.axis).normalize_or(p.axis);
                }
            }
        }
        runtime.anchor = Some(frame);
    }

    /// A spawn offset and launch velocity, worked out in the emitter's own frame,
    /// in world space: turned and scaled by `frame` for a `Local` emitter, as they
    /// are for `World` (`direction` and the shape are world-space there).
    pub(super) fn to_world(&self, frame: &Mat4, offset: Vec3, velocity: Vec3) -> (Vec3, Vec3) {
        match self.simulation_space {
            SimulationSpace::World => (offset, velocity),
            SimulationSpace::Local => (
                frame.transform_vector3(offset),
                frame.transform_vector3(velocity),
            ),
        }
    }
}
