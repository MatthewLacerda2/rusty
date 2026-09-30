//! src/components/particle/spawn.rs — building particles from the emitter config.
//!
//! One spawn path, shared by the sim's continuous/burst emission, sub-emitter
//! firings and the scripting `Particles.Emit`/`Burst`, so all of them draw from
//! the same seeded stream in the same order.

use glam::Vec3;

use super::runtime::{Particle, SubEmitEvent};
use super::shape::signed;
use super::ParticleEmitterComponent;
use crate::core::curve::Range;
use crate::core::random::Random;

/// Sample `range`, spending a draw only when it is not a constant.
fn draw(range: &Range, rng: &mut Random) -> f32 {
    if range.is_constant() {
        range.min
    } else {
        range.sample(rng.value() as f32)
    }
}

impl ParticleEmitterComponent {
    /// Seed the runtime PRNG for the current Play session if it hasn't been yet.
    /// Idempotent — safe to call before any spawn, whether from the sim tick or a
    /// script-driven `Particles.Emit`/`Burst`.
    pub fn ensure_initialized(&mut self) {
        if !self.runtime.initialized {
            self.runtime.rng = Random::new(self.seed);
            self.runtime.initialized = true;
        }
    }

    /// Build one particle at `origin` from the current config: shape offset and
    /// launch direction, direction jitter, then every start range, all drawn from
    /// the seeded stream.
    pub fn spawn_particle(&mut self, origin: Vec3) -> Particle {
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
        // Render extras (#440) draw only when the settings use them, so an emitter
        // that doesn't keeps its seeded stream — and its replays — unchanged.
        let start_frame = if self.render.wants_start_frame() {
            let frames = self.render.flipbook.frames();
            ((rng.value() * f64::from(frames)) as u32).min(frames - 1)
        } else {
            0
        };
        let axis = if self.render.wants_axis() {
            // Uniform on the sphere: z uniform in [-1, 1], azimuth uniform.
            let z = signed(rng);
            let phi = rng.value() as f32 * std::f32::consts::TAU;
            let r = (1.0 - z * z).max(0.0).sqrt();
            Vec3::new(r * phi.cos(), r * phi.sin(), z)
        } else {
            Vec3::Y
        };
        Particle {
            position: origin + offset,
            velocity: dir * speed,
            age: 0.0,
            lifetime,
            size,
            rotation,
            color,
            start_frame,
            axis,
        }
    }

    /// Spawn up to `count` particles at `origin`, honouring the `max_particles`
    /// cap (spawns past the cap are dropped). Returns the number actually spawned.
    /// Seeds the runtime on first use, and queues the birth sub-emitter per spawn.
    pub fn emit_at(&mut self, origin: Vec3, count: u32) -> u32 {
        self.spawn(origin, Vec3::ZERO, count, true)
    }

    /// Spawn a sub-emitter burst: `count` particles at `origin`, each carrying
    /// `inherited` velocity on top of its own. Never queues birth sub-emitters, so
    /// two emitters naming each other cannot feed back without bound.
    pub fn emit_sub(&mut self, origin: Vec3, inherited: Vec3, count: u32) -> u32 {
        self.spawn(origin, inherited, count, false)
    }

    fn spawn(&mut self, origin: Vec3, inherited: Vec3, count: u32, births: bool) -> u32 {
        self.ensure_initialized();
        let cap = self.max_particles as usize;
        let birth = self.sub_emitters.birth.filter(|_| births);
        let inherit = self.sub_emitters.inherit_velocity;
        let mut spawned = 0;
        for _ in 0..count {
            if self.runtime.particles.len() >= cap {
                break;
            }
            let mut p = self.spawn_particle(origin);
            p.velocity += inherited;
            if let Some(target) = birth {
                self.runtime.pending.push(SubEmitEvent {
                    target,
                    position: p.position,
                    velocity: p.velocity * inherit,
                });
            }
            self.runtime.particles.push(p);
            spawned += 1;
        }
        spawned
    }
}
