use glam::Vec3;

use crate::resources::Resources;
use crate::world::World;
use rusty_components::particle::{
    CollisionResponse, EmitMode, EmitterFrame, Particle, ParticleEmitterComponent, SimulationSpace,
    SubEmitEvent,
};
use rusty_physics::PhysicsWorld;

type Physics = std::rc::Rc<std::cell::RefCell<Option<PhysicsWorld>>>;

pub(crate) fn tick_particles(world: &mut World, res: &mut Resources) {
    let dt = res.frame_dt;
    let mut events = Vec::new();
    {
        let mut scene = world.scene.borrow_mut();
        for id in scene.world.ids_with_particles() {
            if !scene.world.is_active_in_hierarchy(id) {
                continue;
            }
            let origin = scene.world.emitter_frame(id);
            let Some(mut emitter) = scene.world.particles_mut(id) else {
                continue;
            };
            simulate_emitter(&mut emitter, origin, dt, &res.physics);
            events.append(&mut emitter.runtime.pending);
        }
    }
    dispatch_sub_emitters(world, &events);
}

pub(crate) fn carry_local_particles(world: &mut World, _res: &mut Resources) {
    let mut scene = world.scene.borrow_mut();
    for id in scene.world.ids_with_particles() {
        let local = scene
            .world
            .particles(id)
            .is_some_and(|p| p.simulation_space == SimulationSpace::Local);
        if !local || !scene.world.is_active_in_hierarchy(id) {
            continue;
        }
        let frame = scene.world.emitter_frame(id);
        if let Some(mut emitter) = scene.world.particles_mut(id) {
            emitter.carry_to(frame);
        }
    }
}

fn dispatch_sub_emitters(world: &mut World, events: &[SubEmitEvent]) {
    if events.is_empty() {
        return;
    }
    let mut scene = world.scene.borrow_mut();
    for e in events {
        if !scene.world.contains(e.target) || !scene.world.is_active_in_hierarchy(e.target) {
            continue;
        }
        let frame = scene.world.emitter_frame(e.target);
        if let Some(mut target) = scene.world.particles_mut(e.target) {
            let count = target.burst_count;
            target.emit_sub(frame, e.position, e.velocity, count);
        }
    }
}

fn simulate_emitter(
    emitter: &mut ParticleEmitterComponent,
    origin: EmitterFrame,
    dt: f32,
    physics: &Physics,
) {
    emitter.ensure_initialized();
    emitter.carry_to(origin);
    emitter.runtime.revision = emitter.runtime.revision.wrapping_add(1);

    if emitter.active {
        emit(emitter, origin, dt);
    }
    if emitter.on_gpu() {
        emitter.runtime.gpu.step(dt);
    } else {
        integrate_and_collide(emitter, dt, physics);
    }
}

fn emit(emitter: &mut ParticleEmitterComponent, origin: EmitterFrame, dt: f32) {
    let to_spawn = match emitter.emit_mode {
        EmitMode::Continuous => {
            emitter.runtime.spawn_accum += emitter.rate * dt;
            let n = emitter.runtime.spawn_accum.floor();
            emitter.runtime.spawn_accum -= n;
            n as u32
        }
        EmitMode::Burst => {
            let can_fire = if emitter.looping {
                emitter.live_count() == 0
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

    emitter.emit_at(origin, to_spawn);
}

fn integrate_and_collide(emitter: &mut ParticleEmitterComponent, dt: f32, physics: &Physics) {
    let (gravity, response) = (emitter.gravity, emitter.collision);
    let (bounciness, subs) = (emitter.bounciness, emitter.sub_emitters);
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

struct Hit {
    point: Vec3,
    survived: bool,
}

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
            *step = dir * (toi * 0.99);
            let n = normal.normalize_or_zero();
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
