//! src/components/particle/space.rs — Unity's **Simulation Space** (#1529).
//!
//! Particles are always stored in world space, so the draw pass, soft particles,
//! lighting, sorting, collisions and sub-emitters read one kind of position. A
//! `Local` emitter keeps them relative to itself by *carrying* them: the runtime
//! remembers the emitter frame the live particles were last placed under (its
//! anchor), and [`ParticleEmitterComponent::carry_to`] moves every live particle
//! by the change from that frame to the emitter's current one — position,
//! velocity, and the mesh orientation's `turn` on top of its own tumble — so
//! they move, turn and scale with the emitter and any parent bone.
//!
//! The emitter frame is Unity's default *Local* Scaling Mode: the entity's world
//! position and rotation under its **own** local scale, every parent's scale
//! ignored (`ecs::World::emitter_frame`), so a viewmodel rig at a 0.01 root scale
//! does not shrink its muzzle flash. The shape and `direction` are in that frame
//! in both spaces, as Unity's Shape module is: a `World` emitter on a muzzle
//! launches along the gun, then leaves its particles behind. Particle size is
//! never scaled. Every spawn carries first and then places the new
//! particles under the same current matrix, so a particle born between ticks (a
//! scripted `Burst` after the gun moved) is never carried by a move that happened
//! before its birth.
//!
//! Unity's third option, `Custom` (relative to another entity), is left out: the
//! scene format has no field kind that references another entity by value yet.

use glam::{Mat4, Quat};

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

/// A frame this close to singular (a zero scale on the emitter) has no usable
/// inverse: carrying to it, and remembering it, is skipped.
const SINGULAR: f32 = 1e-12;

/// The rotation part of a non-singular emitter frame.
fn rotation_of(frame: &Mat4) -> Quat {
    frame.to_scale_rotation_translation().1
}

impl ParticleEmitterComponent {
    /// Bring a `Local` emitter's live particles to the emitter frame `frame`, by
    /// the change from the frame they were last placed under, then remember
    /// `frame`: position and velocity by the whole change, `turn` by its
    /// rotation. A no-op for a `World` emitter, and for a near-singular `frame`
    /// (the anchor is kept, so scaling 1 → 0 → 1 brings the particles back).
    /// Idempotent for one `frame`.
    pub fn carry_to(&mut self, frame: Mat4) {
        if self.simulation_space != SimulationSpace::Local || frame.determinant().abs() <= SINGULAR
        {
            return;
        }
        let runtime = &mut self.runtime;
        if let Some(anchor) = runtime.anchor.filter(|a| *a != frame) {
            let delta = frame * anchor.inverse();
            let turn = rotation_of(&frame) * rotation_of(&anchor).inverse();
            for p in &mut runtime.particles {
                p.position = delta.transform_point3(p.position);
                p.velocity = delta.transform_vector3(p.velocity);
                p.turn = (turn * p.turn).normalize();
            }
        }
        runtime.anchor = Some(frame);
    }

    /// The turn a particle born under `frame` starts with: the frame's rotation
    /// for a `Local` emitter (identity for a singular one), identity for `World`.
    pub(super) fn birth_turn(&self, frame: &Mat4) -> Quat {
        if self.simulation_space == SimulationSpace::Local && frame.determinant().abs() > SINGULAR {
            rotation_of(frame)
        } else {
            Quat::IDENTITY
        }
    }
}
