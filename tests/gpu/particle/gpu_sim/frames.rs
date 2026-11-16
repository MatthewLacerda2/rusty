//! A `Gpu` emitter drawn frame after frame, where the incremental upload and the
//! slot reuse live: it matches its `Cpu` twin at every frame while particles are
//! born and die between them, and a renderer that lags while every particle dies
//! and time stops draws no ghost of the particles it last saw.

use glam::Vec3;
use rusty::components::particle::{EmitMode, EmitShape, ParticleSimulation};
use rusty::core::curve::Range;
use rusty::dev::capture::CaptureHost;

use super::{agreement, emit, emitter, place, red, world};

#[test]
fn a_gpu_emitter_matches_its_cpu_twin_frame_after_frame() {
    let mut host = CaptureHost::new();
    let config = |simulation| {
        let mut e = emitter(simulation);
        e.shape = EmitShape::Sphere { radius: 1.5 };
        e.speed = Range::new(0.2, 1.2);
        e.size = Range::new(0.1, 0.3);
        e.lifetime = Range::new(0.1, 0.4);
        e.gravity = Vec3::new(0.0, -1.0, 0.0);
        e.emit_mode = EmitMode::Continuous;
        e.rate = 300.0;
        e.seed = 2245;
        e
    };
    let (mut gpu, _) = world(config(ParticleSimulation::Gpu));
    let (mut cpu, _) = world(config(ParticleSimulation::Cpu));
    // Every particle lives at most 24 ticks: between two frames 7 ticks apart, a
    // share dies and its slot is reused by a birth the frame never saw alive.
    for frame in 0..8 {
        crate::play::run(&mut gpu, 7);
        crate::play::run(&mut cpu, 7);
        let (Some(a), Some(b)) = (
            red(
                &mut host,
                &gpu.scene().borrow(),
                &format!("frames_gpu_{frame}"),
            ),
            red(
                &mut host,
                &cpu.scene().borrow(),
                &format!("frames_cpu_{frame}"),
            ),
        ) else {
            return;
        };
        assert!(
            a.iter().filter(|&&x| x).count() > 50,
            "frame {frame}: the cloud draws"
        );
        // Measured: 1.0 at every frame on lavapipe (Mesa, the container) and on
        // an NVIDIA RTX 2060 (Vulkan).
        let agree = agreement(&a, &b);
        assert!(agree > 0.98, "frame {frame}: {agree}");
    }
}

#[test]
fn a_lagging_renderer_draws_no_ghost_after_every_particle_died_and_time_stopped() {
    let mut host = CaptureHost::new();
    let (mut game, id) = world(emitter(ParticleSimulation::Gpu));
    let set_lifetime = |game: &rusty::app::GameWorld, seconds: f32| {
        let mut scene = game.scene().borrow_mut();
        scene.world.particles_mut(id).unwrap().lifetime = Range::constant(seconds);
    };
    let left = |mask: &[bool]| (0..mask.len()).any(|i| mask[i] && i % 64 < 28);
    let right = |mask: &[bool]| (0..mask.len()).any(|i| mask[i] && i % 64 > 36);
    // Ten particles on the left, drawn once.
    place(&game, id, Vec3::new(-2.0, 0.0, 0.0));
    set_lifetime(&game, 0.5);
    emit(&game, id, 10);
    crate::play::run(&mut game, 1);
    let Some(seen) = red(&mut host, &game.scene().borrow(), "lag_before") else {
        return;
    };
    assert!(left(&seen), "drawn where they were born");
    // Unseen by the renderer: they all die, ten short-lived ones take their slots
    // and die too, a long-lived one is born on the right, and time stops for longer
    // than the renderer keeps steps.
    crate::play::run(&mut game, 40);
    place(&game, id, Vec3::new(2.0, 0.0, 0.0));
    set_lifetime(&game, 0.05);
    emit(&game, id, 10);
    set_lifetime(&game, 100.0);
    emit(&game, id, 1);
    crate::play::run(&mut game, 5);
    game.time().borrow_mut().time_scale = 0.0;
    crate::play::run(&mut game, 300);
    let count = game
        .scene()
        .borrow()
        .world
        .particles(id)
        .unwrap()
        .live_count();
    assert_eq!(count, 1);
    let Some(now) = red(&mut host, &game.scene().borrow(), "lag_after") else {
        return;
    };
    assert!(right(&now), "the one live particle draws");
    assert!(
        !left(&now),
        "no ghost of the particles last drawn on the left"
    );
}
