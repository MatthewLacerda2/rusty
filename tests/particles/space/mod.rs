//! Simulation Space (#1529): a `Local` emitter carries its live particles when it
//! moves, turns or scales, through the real play loop; a `World` one leaves them.

mod api;
mod frame;
mod inactive;

use std::cell::RefCell;
use std::rc::Rc;

use glam::{Quat, Vec3};
use rusty::app::GameWorld;
use rusty::components::particle::{EmitMode, EmitShape, ParticleEmitterComponent, SimulationSpace};
use rusty::core::curve::Range;
use rusty::scene::Scene;
use rusty::scripting::ConsoleLogs;

/// A one-shot burst of 8 long-lived particles from a box, launched along +X.
fn emitter(space: SimulationSpace, speed: f32) -> ParticleEmitterComponent {
    ParticleEmitterComponent {
        simulation_space: space,
        emit_mode: EmitMode::Burst,
        burst_count: 8,
        looping: false,
        shape: EmitShape::Box { size: Vec3::ONE },
        lifetime: Range::constant(100.0),
        speed: Range::constant(speed),
        direction: Vec3::X,
        spread: 0.0,
        ..Default::default()
    }
}

/// A playing world with one root emitter, ticked once so the burst has fired.
fn burst(space: SimulationSpace, speed: f32) -> (GameWorld, u32) {
    let mut scene = Scene::new();
    let id = scene.add_entity("Emitter".to_string());
    scene.world.set_particles(id, Some(emitter(space, speed)));
    let console = Rc::new(RefCell::new(ConsoleLogs::new()));
    let mut world = crate::play::world(Rc::new(RefCell::new(scene)), 20.0, console);
    world.set_playing(true);
    world.tick(1.0 / 60.0);
    (world, id)
}

/// Every live particle's (position, velocity).
fn state(world: &GameWorld, id: u32) -> Vec<(Vec3, Vec3)> {
    let scene = world.scene().borrow();
    let e = scene.world.particles(id).unwrap();
    e.runtime
        .particles
        .iter()
        .map(|p| (p.position, p.velocity))
        .collect()
}

/// Turn the emitter 90° about Y, move it by `(5, 0, 0)` and scale it by `scale`,
/// then tick once.
fn move_and_tick(world: &mut GameWorld, id: u32, scale: f32) {
    {
        let mut scene = world.scene().borrow_mut();
        let mut t = scene.world.transform_mut(id).unwrap();
        t.position = Vec3::new(5.0, 0.0, 0.0);
        t.rotation = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
        t.scale = Vec3::splat(scale);
    }
    world.tick(1.0 / 60.0);
}

const TURN: Quat = Quat::from_xyzw(
    0.0,
    std::f32::consts::FRAC_1_SQRT_2,
    0.0,
    std::f32::consts::FRAC_1_SQRT_2,
);

#[test]
fn a_local_emitter_carries_its_particles_and_a_world_one_leaves_them() {
    for space in [SimulationSpace::Local, SimulationSpace::World] {
        let (mut world, id) = burst(space, 0.0);
        let before = state(&world, id);
        assert_eq!(before.len(), 8);
        move_and_tick(&mut world, id, 1.0);
        let after = state(&world, id);
        for ((b, _), (a, _)) in before.iter().zip(&after) {
            let carried = TURN * *b + Vec3::new(5.0, 0.0, 0.0);
            let want = if space == SimulationSpace::Local {
                carried
            } else {
                *b
            };
            assert!(a.abs_diff_eq(want, 1e-4), "{space}: {a} != {want}");
        }
    }
}

#[test]
fn velocity_turns_and_scale_applies() {
    let (mut world, id) = burst(SimulationSpace::Local, 1.0);
    let before = state(&world, id);
    move_and_tick(&mut world, id, 2.0);
    let dt = 1.0 / 60.0;
    for ((pb, vb), (pa, va)) in before.iter().zip(&state(&world, id)) {
        let v = TURN * (*vb * 2.0);
        assert!(va.abs_diff_eq(v, 1e-4), "velocity {va} != {v}");
        let p = TURN * (*pb * 2.0) + Vec3::new(5.0, 0.0, 0.0) + v * dt;
        assert!(pa.abs_diff_eq(p, 1e-4), "position {pa} != {p}");
    }
}

#[test]
fn the_same_seed_replays_the_same_particles_in_both_spaces() {
    for space in [SimulationSpace::Local, SimulationSpace::World] {
        let run = || {
            let (mut world, id) = burst(space, 1.0);
            move_and_tick(&mut world, id, 1.5);
            state(&world, id)
        };
        assert_eq!(run(), run(), "{space}");
    }
}
