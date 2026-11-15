//! Where the compute step puts particles: the closed form under gravity, and the
//! same pixels as a `Cpu` emitter of one config and seed.

use glam::Vec3;
use rusty::components::particle::{EmitShape, ParticleSimulation};
use rusty::core::curve::Range;
use rusty::dev::capture::CaptureHost;

use super::super::scene::{one, scene_with};
use super::{agreement, centroid, emit, emitter, red, world};

#[test]
fn a_gpu_particle_under_gravity_lands_where_the_closed_form_says() {
    let mut host = CaptureHost::new();
    let (v0, g, n) = (Vec3::new(3.0, 1.0, 0.0), Vec3::new(0.0, -4.0, 0.0), 40usize);
    let mut e = emitter(ParticleSimulation::Gpu);
    e.direction = v0.normalize();
    e.speed = Range::constant(v0.length());
    e.gravity = g;
    let (mut game, id) = world(e);
    assert_eq!(emit(&game, id, 1), 1);
    crate::play::run(&mut game, n);
    // Semi-implicit Euler, one step per tick: x_n = n·dt·v0 + dt²·g·n(n+1)/2.
    let (dt, nf) = (crate::play::DT, n as f32);
    let at = v0 * nf * dt + g * dt * dt * nf * (nf + 1.0) / 2.0;
    let (reference, _) = scene_with(Default::default(), one(at, 0.6));
    let scene = game.scene().borrow();
    let (Some(gpu), Some(want)) = (
        red(&mut host, &scene, "closed"),
        red(&mut host, &reference, "closed_ref"),
    ) else {
        return;
    };
    let (g, w) = (
        centroid(&gpu).expect("drawn"),
        centroid(&want).expect("drawn"),
    );
    // Measured on lavapipe (Mesa, the container): centroids 0.0 px apart and the
    // masks agree at 1.0; a desktop GPU's values are owed before merge.
    assert!(
        (g.0 - w.0).abs() < 0.75 && (g.1 - w.1).abs() < 0.75,
        "{g:?} vs {w:?}"
    );
    assert!(agreement(&gpu, &want) > 0.99);
}

#[test]
fn a_gpu_and_a_cpu_emitter_of_one_seed_cover_the_same_pixels() {
    let mut host = CaptureHost::new();
    let config = |simulation| {
        let mut e = emitter(simulation);
        e.shape = EmitShape::Sphere { radius: 1.5 };
        e.speed = Range::new(0.2, 1.2);
        e.size = Range::new(0.1, 0.3);
        e.lifetime = Range::new(0.3, 1.2);
        e.gravity = Vec3::new(0.0, -1.0, 0.0);
        e.emit_mode = rusty::components::particle::EmitMode::Continuous;
        e.rate = 200.0;
        e.seed = 2170;
        e
    };
    let (mut gpu, gpu_id) = world(config(ParticleSimulation::Gpu));
    let (mut cpu, cpu_id) = world(config(ParticleSimulation::Cpu));
    for tick in 0..50 {
        crate::play::run(&mut gpu, 1);
        crate::play::run(&mut cpu, 1);
        let count = |g: &rusty::app::GameWorld, id| {
            g.scene().borrow().world.particles(id).unwrap().live_count()
        };
        assert_eq!(count(&gpu, gpu_id), count(&cpu, cpu_id), "tick {tick}");
    }
    let (Some(a), Some(b)) = (
        red(&mut host, &gpu.scene().borrow(), "twin_gpu"),
        red(&mut host, &cpu.scene().borrow(), "twin_cpu"),
    ) else {
        return;
    };
    assert!(a.iter().filter(|&&x| x).count() > 100, "the cloud draws");
    // Measured on lavapipe (Mesa, the container): 1.0.
    assert!(agreement(&a, &b) > 0.98, "{}", agreement(&a, &b));
}
