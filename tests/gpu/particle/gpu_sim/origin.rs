//! A floating-origin shift moves a world-space GPU emitter's particles: those already on
//! the GPU, those born before the next upload and those born after it, so the frame after
//! the shift draws what its `Cpu` twin draws, in the same place.

use glam::{IVec3, Vec3};
use rusty::components::particle::{
    EmitMode, EmitShape, ParticleEmitterComponent, ParticleSimulation,
};
use rusty::core::curve::Range;
use rusty::dev::capture::CaptureHost;

use super::{agreement, emitter, world};
use crate::gpu::fog::scene::camera;
use crate::gpu::particle::scene::is_red;
use rusty::dev::screenshot::capture_into;

/// The fog tests' view, moved with the World, as a red mask.
fn red(host: &mut CaptureHost, game: &rusty::app::GameWorld, name: &str) -> Option<Vec<bool>> {
    let scene = game.scene().borrow();
    let mut view = camera();
    view.position -= scene.loaded.origin.offset_f32();
    let path = crate::temp::dir().join(format!("rusty_gpu_sim_{name}.png"));
    if !capture_into(host, &scene, &view, &path, 64, 64).expect("capture") {
        return None;
    }
    let img = image::open(&path).expect("png").to_rgb8();
    Some(img.pixels().map(|p| is_red(p.0)).collect())
}

fn config(simulation: ParticleSimulation) -> ParticleEmitterComponent {
    let mut e = emitter(simulation);
    e.shape = EmitShape::Sphere { radius: 0.5 };
    e.direction = Vec3::X;
    e.speed = Range::new(0.3, 0.8);
    e.size = Range::new(0.15, 0.3);
    e.emit_mode = EmitMode::Continuous;
    e.rate = 40.0;
    e.seed = 2507;
    e
}

#[test]
fn a_shift_moves_what_the_gpu_holds_with_the_world() {
    let mut host = CaptureHost::new();
    let (mut gpu, gpu_id) = world(config(ParticleSimulation::Gpu));
    let (mut cpu, _) = world(config(ParticleSimulation::Cpu));
    for game in [&mut gpu, &mut cpu] {
        crate::play::run(game, 60);
    }
    let Some(before) = red(&mut host, &gpu, "origin_gpu_before") else {
        return;
    };
    for game in [&mut gpu, &mut cpu] {
        game.scene().borrow_mut().shift_origin(IVec3::new(1, 0, -2));
        crate::play::run(game, 2);
    }
    let gpu_sim = gpu
        .scene()
        .borrow()
        .world
        .particles(gpu_id)
        .unwrap()
        .runtime
        .gpu
        .moved();
    assert_eq!(
        gpu_sim,
        Vec3::new(-1024.0, 0.0, 2048.0),
        "the sim kept the shift"
    );
    let (Some(a), Some(b)) = (
        red(&mut host, &gpu, "origin_gpu_after"),
        red(&mut host, &cpu, "origin_cpu_after"),
    ) else {
        return;
    };
    assert!(a.iter().filter(|&&x| x).count() > 50, "still drawn");
    // Measured on lavapipe.
    let agree = agreement(&a, &b);
    assert!(
        agree > 0.98,
        "the GPU draws what the CPU twin draws: {agree}"
    );
    assert!(
        agreement(&before, &a) > 0.9,
        "where they were, two ticks on"
    );
}
