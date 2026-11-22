//! One spawn path for the sim's emission, sub-emitters and `Particles.Emit`/`Burst`, so all
//! draw from the same seeded stream in the same order.

use glam::{Mat4, Vec3};

use super::runtime::{Particle, SubEmitEvent};
use super::shape::signed;
use super::space::{EmitterFrame, SimulationSpace};
use super::ParticleEmitterComponent;
use rusty_core::curve::Range;
use rusty_core::random::Random;

/// Sample `range`, spending a draw only when it is not a constant.
fn draw(range: &Range, rng: &mut Random) -> f32 {
    if range.is_constant() {
        range.min
    } else {
        range.sample(rng.value() as f32)
    }
}

impl ParticleEmitterComponent {
    pub fn ensure_initialized(&mut self) {
        if !self.runtime.initialized {
            self.runtime.rng = Random::new(self.seed);
            self.runtime.initialized = true;
        }
    }

    fn spawn_particle(&mut self, frame: &EmitterFrame, origin: Vec3) -> Particle {
        let axis = self.direction.normalize_or_zero();
        let rng = &mut self.runtime.rng;
        let (offset, launch) = self.shape.sample(self.emit_from, axis, rng);
        let jitter = if self.spread > 0.0 {
            Vec3::new(signed(rng), signed(rng), signed(rng)) * self.spread
        } else {
            Vec3::ZERO
        };
        let dir = (launch + jitter).normalize_or_zero();
        let speed = draw(&self.speed, rng);
        let lifetime = draw(&self.lifetime, rng);
        let size = draw(&self.size, rng);
        let rotation = draw(&self.rotation, rng).to_radians();
        let color = if self.color.is_constant() {
            self.color.min
        } else {
            self.color.sample(rng.value() as f32)
        };
        // Render extras draw only when the settings use them, so an emitter
        // that doesn't keeps its seeded stream — and its replays — unchanged.
        let start_frame = if self.render.wants_start_frame() {
            let frames = self.render.flipbook.frames();
            ((rng.value() * f64::from(frames)) as u32).min(frames - 1)
        } else {
            0
        };
        let axis = if self.render.wants_axis() {
            let z = signed(rng);
            let phi = rng.value() as f32 * std::f32::consts::TAU;
            let r = (1.0 - z * z).max(0.0).sqrt();
            Vec3::new(r * phi.cos(), r * phi.sin(), z)
        } else {
            Vec3::Y
        };
        Particle {
            position: origin + frame.matrix.transform_vector3(offset),
            velocity: frame.matrix.transform_vector3(dir * speed),
            age: 0.0,
            lifetime,
            size,
            rotation,
            color,
            start_frame,
            axis,
            turn: self.birth_turn(frame),
            custom: [0.0; 4],
        }
    }

    pub fn placed(&self, frame: &EmitterFrame) -> Particle {
        Particle {
            turn: self.birth_turn(frame),
            ..Particle::at(
                frame.origin(),
                self.size.min,
                self.color.min,
                self.lifetime.min,
            )
        }
    }

    pub fn emit_at(&mut self, frame: EmitterFrame, count: u32) -> u32 {
        self.spawn(frame, frame.origin(), Vec3::ZERO, count, true)
    }

    /// Never queues birth sub-emitters, so two emitters naming each other cannot feed back
    /// unbounded.
    pub fn emit_sub(
        &mut self,
        frame: EmitterFrame,
        origin: Vec3,
        inherited: Vec3,
        count: u32,
    ) -> u32 {
        self.spawn(frame, origin, inherited, count, false)
    }

    fn spawn(
        &mut self,
        frame: EmitterFrame,
        origin: Vec3,
        inherited: Vec3,
        count: u32,
        births: bool,
    ) -> u32 {
        self.ensure_initialized();
        // Carry first: a particle born now is never carried by an earlier move, and `gpu_space`
        // below reads the new anchor.
        self.carry_to(frame);
        let cap = self.particle_cap() as usize;
        let birth = self.sub_emitters.birth.filter(|_| births);
        let inherit = self.sub_emitters.inherit_velocity;
        let mut spawned = 0;
        let to_local = (self.on_gpu() && self.simulation_space == SimulationSpace::Local)
            .then(|| self.gpu_space())
            .filter(|m| *m != Mat4::IDENTITY)
            .map(|m| m.inverse())
            .filter(|m| m.is_finite());
        for _ in 0..count {
            if self.live_count() >= cap {
                break;
            }
            let mut p = self.spawn_particle(&frame, origin);
            p.velocity += inherited;
            if let Some(target) = birth {
                self.runtime.pending.push(SubEmitEvent {
                    target,
                    position: p.position,
                    velocity: p.velocity * inherit,
                });
            }
            if !self.on_gpu() {
                self.runtime.particles.push(p);
            } else if let Some(m) = to_local {
                p.position = m.transform_point3(p.position);
                p.velocity = m.transform_vector3(p.velocity);
                self.runtime.gpu.add(p);
            } else {
                self.runtime.gpu.add(p);
            }
            spawned += 1;
        }
        if spawned > 0 {
            self.runtime.revision = self.runtime.revision.wrapping_add(1);
        }
        spawned
    }
}

#[cfg(test)]
#[path = "spawn_tests.rs"]
mod tests;
