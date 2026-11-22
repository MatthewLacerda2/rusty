//! `Cpu` particles are always stored in world space, so every reader sees one kind of position;
//! a `Local` emitter carries them.

use glam::{Mat4, Quat, Vec3};

use super::ParticleEmitterComponent;

rusty_core::named_enum! {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum SimulationSpace {
        #[default]
        World = "World",
        Local = "Local",
    }
}

/// A determinant this close to 0 (a zero scale) has no usable inverse: carries skip the frame.
const SINGULAR: f32 = 1e-12;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EmitterFrame {
    pub matrix: Mat4,
    /// What `matrix` was built with, so a carry turns particles without decomposing it.
    pub rotation: Quat,
}

impl EmitterFrame {
    /// The world origin, unturned and unscaled.
    pub const IDENTITY: Self = Self {
        matrix: Mat4::IDENTITY,
        rotation: Quat::IDENTITY,
    };

    pub fn new(position: Vec3, rotation: Quat, scale: Vec3) -> Self {
        Self {
            matrix: Mat4::from_scale_rotation_translation(scale, rotation, position),
            rotation,
        }
    }

    pub fn origin(&self) -> Vec3 {
        self.matrix.w_axis.truncate()
    }

    fn singular(&self) -> bool {
        self.matrix.determinant().abs() <= SINGULAR
    }
}

impl ParticleEmitterComponent {
    pub fn carry_to(&mut self, frame: EmitterFrame) {
        if self.simulation_space != SimulationSpace::Local || frame.singular() {
            return;
        }
        let runtime = &mut self.runtime;
        if let Some(anchor) = runtime.anchor.filter(|a| *a != frame) {
            let delta = frame.matrix * anchor.matrix.inverse();
            let turn = frame.rotation * anchor.rotation.inverse();
            for p in &mut runtime.particles {
                p.position = delta.transform_point3(p.position);
                p.velocity = delta.transform_vector3(p.velocity);
                p.turn = (turn * p.turn).normalize();
            }
        }
        runtime.anchor = Some(frame);
        runtime.base.get_or_insert(frame);
    }

    /// Births are stored under its inverse, relative to the first invertible frame (`base`).
    pub fn gpu_space(&self) -> Mat4 {
        match (
            self.simulation_space,
            self.runtime.anchor,
            self.runtime.base,
        ) {
            (SimulationSpace::Local, Some(anchor), Some(base)) if anchor != base => {
                anchor.matrix * base.matrix.inverse()
            }
            _ => Mat4::IDENTITY,
        }
    }

    /// The turn a particle born under `frame` starts with: the frame's rotation
    /// for a `Local` emitter (identity for a singular one), identity for `World`.
    pub(super) fn birth_turn(&self, frame: &EmitterFrame) -> Quat {
        if self.simulation_space == SimulationSpace::Local && !frame.singular() {
            frame.rotation
        } else {
            Quat::IDENTITY
        }
    }
}
