//! A `Local` GPU emitter's particles follow it, and a dead particle's slot draws
//! nothing while a live one keeps the slots in use.

use glam::Vec3;
use rusty::components::particle::{ParticleSimulation, SimulationSpace};
use rusty::core::curve::Range;
use rusty::dev::capture::CaptureHost;

use super::{centroid, emit, emitter, place, red, world};

#[test]
fn a_local_gpu_emitter_carries_its_particles_and_a_world_one_leaves_them() {
    let mut host = CaptureHost::new();
    let mut seen = Vec::new();
    for space in [SimulationSpace::Local, SimulationSpace::World] {
        let mut e = emitter(ParticleSimulation::Gpu);
        e.simulation_space = space;
        let (mut game, id) = world(e);
        emit(&game, id, 1);
        crate::play::run(&mut game, 2);
        place(&game, id, Vec3::new(2.0, 0.0, 0.0));
        crate::play::run(&mut game, 1);
        let Some(mask) = red(&mut host, &game.scene().borrow(), &format!("{space:?}")) else {
            return;
        };
        seen.push(centroid(&mask).expect("drawn").0);
    }
    assert!(
        seen[0] > 40.0,
        "Local moved right with the emitter: {seen:?}"
    );
    assert!(
        (seen[1] - 31.5).abs() < 1.5,
        "World stayed at the centre: {seen:?}"
    );
}

#[test]
fn a_dead_particle_is_not_drawn_beside_a_live_one() {
    let mut host = CaptureHost::new();
    let (mut game, id) = world(emitter(ParticleSimulation::Gpu));
    let set_life = |s: f32| {
        let mut scene = game.scene().borrow_mut();
        scene.world.particles_mut(id).unwrap().lifetime = Range::constant(s);
    };
    set_life(0.1);
    emit(&game, id, 1); // slot 0, at the centre, dies after 6 ticks
    set_life(10.0);
    place(&game, id, Vec3::new(-2.0, 0.0, 0.0));
    emit(&game, id, 1); // slot 1, left, lives on
    let Some(before) = red(&mut host, &game.scene().borrow(), "both") else {
        return;
    };
    assert!(
        before[32 * 64 + 32],
        "the short-lived one draws while alive"
    );
    crate::play::run(&mut game, 20);
    let after = red(&mut host, &game.scene().borrow(), "one").expect("adapter");
    assert!(!after[32 * 64 + 32], "its slot draws nothing once dead");
    assert!(centroid(&after).expect("the other draws").0 < 25.0);
    assert_eq!(
        game.scene()
            .borrow()
            .world
            .particles(id)
            .unwrap()
            .live_count(),
        1
    );
}
