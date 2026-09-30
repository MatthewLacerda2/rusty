//! src/app/particles.rs — the particle simulation tick (issue #13, #439).
//!
//! Pure CPU. Reads each entity's `ParticleEmitterComponent`, emits new particles
//! (continuous stream or one burst), integrates position/velocity under gravity
//! and the drag curve, spins them by the rotation-speed curve, ages + despawns
//! them, and resolves point collisions against the rapier world by ray-casting
//! each particle's motion segment. Births, deaths and collisions queue sub-emitter
//! firings, dispatched once every emitter has run. It produces only particle
//! *state*; the draw pass reads it. The sim never touches the GPU — that
//! decoupling is what keeps headless play deterministic.
//!
//! Determinism: all randomness comes from each emitter's seeded stream (reset from
//! `seed` on the first tick of a Play session) and emitters run in entity order,
//! so a fixed-timestep replay is bit-for-bit identical.

use glam::Vec3;

use super::resources::Resources;
use super::world::World;
use crate::components::particle::{
    CollisionResponse, EmitMode, Particle, ParticleEmitterComponent, SubEmitEvent,
};
use crate::physics::PhysicsWorld;

type Physics = std::rc::Rc<std::cell::RefCell<Option<PhysicsWorld>>>;

/// Advance every emitter in the scene by `res.dt()`. The one localised call wired
/// into the play loop, a canonical `fn(&mut World, &mut Resources)` system.
pub(super) fn tick_particles(world: &mut World, res: &mut Resources) {
    let dt = res.frame_dt;
    let ids = world.scene.borrow().world.ids_with_particles();
    let mut events = Vec::new();
    for id in ids {
        // Pull the emitter + spawn origin out, simulate, then write it back. Taking
        // the component out avoids holding the scene borrow across the physics
        // ray-casts (which immutably borrow the scene through the filter).
        let taken = {
            let mut scene = world.scene.borrow_mut();
            if !scene.world.contains(id) || !scene.world.is_active(id) {
                continue;
            }
            let Some(origin) = scene.world.transform(id).map(|t| t.position) else {
                continue;
            };
            scene
                .world
                .take_particles(id)
                .map(|emitter| (emitter, origin))
        };
        let Some((mut emitter, origin)) = taken else {
            continue;
        };

        simulate_emitter(&mut emitter, origin, dt, &res.physics);
        events.append(&mut emitter.runtime.pending);

        world
            .scene
            .borrow_mut()
            .world
            .set_particles(id, Some(emitter));
    }
    dispatch_sub_emitters(world, &events);
}

/// Fire each queued sub-emitter: the target emits its `burst_count` at the
/// event's position. A target whose entity is gone, inactive or has no emitter is
/// skipped. The new particles first move on the next tick.
fn dispatch_sub_emitters(world: &mut World, events: &[SubEmitEvent]) {
    if events.is_empty() {
        return;
    }
    let mut scene = world.scene.borrow_mut();
    for e in events {
        if !scene.world.contains(e.target) || !scene.world.is_active(e.target) {
            continue;
        }
        if let Some(mut target) = scene.world.particles_mut(e.target) {
            let count = target.burst_count;
            target.emit_sub(e.position, e.velocity, count);
        }
    }
}

/// Run one emitter's spawn + integrate + collide + age pipeline for this frame.
fn simulate_emitter(
    emitter: &mut ParticleEmitterComponent,
    origin: Vec3,
    dt: f32,
    physics: &Physics,
) {
    emitter.ensure_initialized();

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

    // Spawning (shape + ranges + cap) lives on the component so the scripting
    // `Particles` namespace shares the exact same deterministic path.
    emitter.emit_at(origin, to_spawn);
}

/// Integrate each particle (gravity, drag, spin, velocity), resolve a collision
/// against the physics world if any, age it, drop the dead, and queue the death /
/// collision sub-emitters. Retains particle order.
fn integrate_and_collide(emitter: &mut ParticleEmitterComponent, dt: f32, physics: &Physics) {
    let (gravity, response) = (emitter.gravity, emitter.collision);
    let (bounciness, subs) = (emitter.bounciness, emitter.sub_emitters);
    // Disjoint field borrows: the curves are read while the runtime is mutated.
    let (drag, spin) = (&emitter.drag, &emitter.rotation_speed);
    let phys = physics.borrow();
    let runtime = &mut emitter.runtime;
    let pending = &mut runtime.pending;
    let fire = |target: Option<u32>, p: &Particle, pending: &mut Vec<SubEmitEvent>| {
        if let Some(target) = target {
            pending.push(SubEmitEvent {
                target,
                position: p.position,
                velocity: p.velocity * subs.inherit_velocity,
            });
        }
    };

    runtime.particles.retain_mut(|p| {
        let t = p.life_t();
        p.velocity += gravity * dt;
        p.velocity *= (1.0 - drag.evaluate(t) * dt).max(0.0);
        p.rotation += spin.evaluate(t).to_radians() * dt;
        let mut step = p.velocity * dt;

        let mut alive = true;
        if response != CollisionResponse::None {
            if let Some(world) = phys.as_ref() {
                if let Some(hit) = resolve_collision(p, &mut step, world, response, bounciness) {
                    alive = hit.survived;
                    let at = Particle {
                        position: hit.point,
                        ..*p
                    };
                    fire(subs.collision, &at, pending);
                }
            }
        }
        p.position += step;

        p.age += dt;
        let alive = alive && p.age < p.lifetime;
        if !alive {
            fire(subs.death, p, pending);
        }
        alive
    });
}

/// A particle's contact this frame: where it hit and whether it lives on.
struct Hit {
    point: Vec3,
    survived: bool,
}

/// Cast the particle's motion segment against the rapier world. On hit either
/// despawn (`Die`) or reflect the velocity about the surface normal (`Bounce`).
/// Returns the contact, or `None` for no hit. `step` is clamped to the hit point
/// so the particle never tunnels through the collider.
fn resolve_collision(
    p: &mut Particle,
    step: &mut Vec3,
    world: &PhysicsWorld,
    response: CollisionResponse,
    bounciness: f32,
) -> Option<Hit> {
    let dist = step.length();
    if dist <= f32::EPSILON {
        return None;
    }
    let dir = *step / dist;
    let (toi, normal) = world
        .cast_ray_with_normal(p.position, dir, dist)
        .filter(|(toi, _)| *toi <= dist)?;
    let point = p.position + dir * toi;

    let survived = match response {
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
    };
    Some(Hit { point, survived })
}
