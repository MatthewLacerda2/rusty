//! src/app/particles.rs — the particle simulation tick (issue #13).
//!
//! Pure CPU. Reads each entity's `ParticleEmitterComponent`, emits new particles
//! (continuous stream or one burst), integrates position/velocity under gravity,
//! ages + despawns them, and resolves point collisions against the rapier world by
//! ray-casting each particle's motion segment. It produces only particle *state*;
//! the draw pass (`render::particles`) reads it. The sim never touches the GPU —
//! that decoupling is what keeps headless play deterministic.
//!
//! Determinism: all randomness comes from the emitter's seeded `ParticleRng`
//! (reset from `seed` on the first tick of a Play session). No wall-clock reads, no
//! unseeded RNG — so a fixed-timestep replay is bit-for-bit identical.

use glam::Vec3;

use super::resources::Resources;
use super::world::World;
use crate::components::particle::{
    CollisionResponse, EmitMode, Particle, ParticleEmitterComponent, ParticleRng,
};
use crate::physics::PhysicsWorld;

/// Advance every emitter in the scene by `res.dt()`. The one localised call wired
/// into the play loop, now a canonical `fn(&mut World, &mut Resources)` system.
pub(super) fn tick_particles(world: &mut World, res: &mut Resources) {
    let dt = res.frame_dt;
    let ids = world.scene.borrow().entity_ids();
    for id in ids {
        // Pull the emitter + spawn origin out, simulate, then write it back. Taking
        // the component out avoids holding the scene borrow across the physics
        // ray-casts (which immutably borrow the scene through the filter).
        let taken = {
            let mut scene = world.scene.borrow_mut();
            let mut entity = match scene.get_entity_mut(id) {
                Some(e) => e,
                None => continue,
            };
            if !entity.active {
                continue;
            }
            entity
                .particles
                .take()
                .map(|emitter| (emitter, entity.transform.position))
        };
        let (mut emitter, origin) = match taken {
            Some(v) => v,
            None => continue,
        };

        simulate_emitter(&mut emitter, origin, dt, &res.physics);

        if let Some(mut entity) = world.scene.borrow_mut().get_entity_mut(id) {
            entity.particles = Some(emitter);
        }
    }
}

/// Run one emitter's spawn + integrate + collide + age pipeline for this frame.
fn simulate_emitter(
    emitter: &mut ParticleEmitterComponent,
    origin: Vec3,
    dt: f32,
    physics: &std::rc::Rc<std::cell::RefCell<Option<PhysicsWorld>>>,
) {
    if !emitter.runtime.initialized {
        emitter.runtime.rng = ParticleRng::new(emitter.seed);
        emitter.runtime.initialized = true;
    }

    if emitter.active {
        emit(emitter, origin, dt);
    }
    integrate_and_collide(emitter, dt, physics);
}

/// Spawn new particles for this frame per the emit mode (continuous rate or a
/// one/looping burst), honouring the max-particle cap.
fn emit(emitter: &mut ParticleEmitterComponent, origin: Vec3, dt: f32) {
    let to_spawn = match emitter.emit_mode {
        EmitMode::Continuous => {
            emitter.runtime.spawn_accum += emitter.rate * dt;
            let n = emitter.runtime.spawn_accum.floor();
            emitter.runtime.spawn_accum -= n;
            n as u32
        }
        EmitMode::Burst => {
            // Fire the full burst once. A looping burst re-fires only after the
            // previous batch has fully expired (periodic explosions); a one-shot
            // burst never re-fires. This avoids the "burst every frame" firehose.
            let can_fire = if emitter.looping {
                emitter.runtime.particles.is_empty()
            } else {
                !emitter.runtime.burst_fired
            };
            if can_fire {
                emitter.runtime.burst_fired = true;
                emitter.burst_count
            } else {
                0
            }
        }
    };

    let cap = emitter.max_particles as usize;
    for _ in 0..to_spawn {
        if emitter.runtime.particles.len() >= cap {
            break;
        }
        let p = spawn_one(emitter, origin);
        emitter.runtime.particles.push(p);
    }
}

/// Build one particle from the emitter config + a draw from the seeded PRNG.
fn spawn_one(emitter: &mut ParticleEmitterComponent, origin: Vec3) -> Particle {
    let rng = &mut emitter.runtime.rng;
    let base = emitter.direction.normalize_or_zero();
    // Cone spread: perturb the base direction by a random offset scaled by spread.
    let jitter = Vec3::new(rng.signed(), rng.signed(), rng.signed()) * emitter.spread;
    let dir = (base + jitter).normalize_or_zero();
    let velocity = if dir == Vec3::ZERO {
        Vec3::ZERO
    } else {
        dir * emitter.speed
    };
    Particle {
        position: origin,
        velocity,
        age: 0.0,
        lifetime: emitter.lifetime,
        size_start: emitter.size_start,
        size_end: emitter.size_end,
        color: emitter.color,
    }
}

/// Integrate each particle (gravity + velocity), resolve a collision against the
/// physics world if any, age it, and drop the dead. Retains particle order.
fn integrate_and_collide(
    emitter: &mut ParticleEmitterComponent,
    dt: f32,
    physics: &std::rc::Rc<std::cell::RefCell<Option<PhysicsWorld>>>,
) {
    let gravity = emitter.gravity;
    let response = emitter.collision;
    let bounciness = emitter.bounciness;
    let phys = physics.borrow();

    emitter.runtime.particles.retain_mut(|p| {
        p.velocity += gravity * dt;
        let mut step = p.velocity * dt;

        let mut alive = true;
        if response != CollisionResponse::None {
            if let Some(world) = phys.as_ref() {
                alive = resolve_collision(p, &mut step, world, response, bounciness);
            }
        }
        p.position += step;

        p.age += dt;
        alive && p.age < p.lifetime
    });
}

/// Cast the particle's motion segment against the rapier world. On hit either
/// despawn (`Die`) or reflect the velocity about the surface normal (`Bounce`).
/// Returns `false` if the particle should be removed. `step` is clamped to the hit
/// point so the particle never tunnels through the collider.
fn resolve_collision(
    p: &mut Particle,
    step: &mut Vec3,
    world: &PhysicsWorld,
    response: CollisionResponse,
    bounciness: f32,
) -> bool {
    let dist = step.length();
    if dist <= f32::EPSILON {
        return true;
    }
    let dir = *step / dist;
    let (toi, normal) = match world.cast_ray_with_normal(p.position, dir, dist) {
        Some((toi, n)) if toi <= dist => (toi, n),
        _ => return true,
    };

    match response {
        CollisionResponse::Die => false,
        CollisionResponse::Bounce => {
            // Advance to just shy of the surface, then reflect the velocity about
            // the hit normal (v' = v - 2(v·n)n) scaled by restitution.
            *step = dir * (toi * 0.99);
            let n = normal.normalize_or_zero();
            // Degenerate normal (no usable surface): nudge the particle back out
            // along its travel and reverse, rather than reflect about a zero vector
            // (which would leave the velocity unchanged and re-collide every frame).
            let reflected = if n == Vec3::ZERO {
                -p.velocity
            } else {
                p.velocity - 2.0 * p.velocity.dot(n) * n
            };
            p.velocity = reflected * bounciness;
            true
        }
        CollisionResponse::None => true,
    }
}
