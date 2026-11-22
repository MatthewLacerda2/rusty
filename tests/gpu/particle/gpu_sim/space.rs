//! A `Local` GPU emitter's particles follow it (through a scale of 0, as its `Cpu`
//! twin does); a dead particle's slot draws nothing while a live one keeps its slot.

use glam::Vec3;
use rusty::components::particle::{ParticleSimulation, SimulationSpace};
use rusty::core::curve::Range;
use rusty::dev::capture::CaptureHost;

use super::{agreement, centroid, count, emit, emitter, place, red, world};

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
    // Centroid x in pixels; lavapipe measures Local at 60.5, World at 31.5.
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

#[test]
fn a_local_gpu_emitter_scaling_through_0_while_emitting_matches_its_cpu_twin() {
    let mut host = CaptureHost::new();
    let config = |simulation| {
        let mut e = emitter(simulation);
        e.simulation_space = SimulationSpace::Local;
        e.shape = rusty::components::particle::EmitShape::Sphere { radius: 1.0 };
        e.speed = Range::new(0.2, 0.6);
        e.size = Range::constant(0.8);
        e.lifetime = Range::new(2.0, 3.0);
        e.emit_mode = rusty::components::particle::EmitMode::Continuous;
        e.rate = 60.0;
        e
    };
    let (mut gpu, gpu_id) = world(config(ParticleSimulation::Gpu));
    let (mut cpu, cpu_id) = world(config(ParticleSimulation::Cpu));
    // Off the origin: a world-space birth drawn under the frame lands twice as far.
    let at = Vec3::new(3.0, 0.0, -10.0);
    for (frame, scale) in [1.0, 0.0, 1.0].into_iter().enumerate() {
        for (game, id) in [(&mut gpu, gpu_id), (&mut cpu, cpu_id)] {
            place(game, id, at);
            game.scene()
                .borrow_mut()
                .world
                .transform_mut(id)
                .unwrap()
                .scale = Vec3::splat(scale);
            crate::play::run(game, 30);
        }
        assert_eq!(count(&gpu, gpu_id), count(&cpu, cpu_id), "frame {frame}");
        let (Some(a), Some(b)) = (
            red(
                &mut host,
                &gpu.scene().borrow(),
                &format!("scale_gpu_{frame}"),
            ),
            red(
                &mut host,
                &cpu.scene().borrow(),
                &format!("scale_cpu_{frame}"),
            ),
        ) else {
            return;
        };
        assert!(
            b.iter().filter(|&&x| x).count() > 30,
            "frame {frame}: drawn"
        );
        // Measured on lavapipe: 1.0 at every frame.
        let agree = agreement(&a, &b);
        assert!(agree > 0.98, "frame {frame}: {agree}");
    }
}
