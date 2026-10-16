//! Shapes, ranges and curves keep the seeded replay bit-for-bit, and every start
//! value lands inside its range.

use glam::Vec3;
use rusty::components::particle::{EmitFrom, EmitMode, EmitShape, ParticleEmitterComponent};
use rusty::core::curve::{ColorRange, Curve, Gradient, Range};
use rusty::scene::Scene;

use super::play;

/// Everything #439 added, turned on at once.
fn full_emitter(seed: u64) -> ParticleEmitterComponent {
    ParticleEmitterComponent {
        emit_mode: EmitMode::Continuous,
        rate: 40.0,
        shape: EmitShape::Cone {
            angle: 30.0,
            radius: 0.5,
        },
        emit_from: EmitFrom::Volume,
        lifetime: Range::new(0.5, 2.0),
        speed: Range::new(1.0, 4.0),
        size: Range::new(0.1, 0.3),
        rotation: Range::new(-90.0, 90.0),
        color: ColorRange {
            min: [1.0, 0.2, 0.0, 1.0],
            max: [1.0, 0.8, 0.2, 1.0],
        },
        size_over_life: Curve::from_keys(&[(0.0, 0.5), (0.2, 1.5), (1.0, 0.0)]),
        color_over_life: Gradient::fade_out(),
        drag: Curve::linear(0.0, 2.0),
        rotation_speed: Curve::constant(180.0),
        gravity: Vec3::new(0.0, -9.81, 0.0),
        seed,
        ..Default::default()
    }
}

/// Every live particle's full visible state after `frames` ticks.
fn run(seed: u64, frames: u32) -> Vec<(Vec3, Vec3, f32, f32, [f32; 4], f32)> {
    let mut scene = Scene::new();
    let id = scene.add_entity("Emitter".to_string());
    scene.world.set_particles(id, Some(full_emitter(seed)));
    let world = play(scene, frames);
    let w = world.borrow();
    let scene = w.scene().borrow();
    let e = scene.world.particles(id).unwrap();
    let rows = e.runtime.particles.iter();
    rows.map(|p| {
        (
            p.position,
            p.velocity,
            e.size_of(p),
            p.rotation,
            e.color_of(p),
            p.lifetime,
        )
    })
    .collect()
}

#[test]
fn shapes_ranges_and_curves_replay_identically() {
    let a = run(42, 90);
    assert!(a.len() > 10, "the emitter should be streaming");
    assert_eq!(a, run(42, 90), "same seed must replay bit-for-bit");
    assert_ne!(a, run(43, 90), "a different seed must change the stream");
}

#[test]
fn start_values_stay_inside_their_ranges() {
    let mut scene = Scene::new();
    let id = scene.add_entity("Emitter".to_string());
    let mut cfg = full_emitter(5);
    cfg.emit_mode = EmitMode::Burst;
    cfg.burst_count = 200;
    cfg.max_particles = 200;
    cfg.looping = false;
    scene.world.set_particles(id, Some(cfg));
    let world = play(scene, 1);
    let w = world.borrow();
    let scene = w.scene().borrow();
    let e = scene.world.particles(id).unwrap();
    assert_eq!(e.live_count(), 200);
    let (mut short, mut long) = (false, false);
    for p in &e.runtime.particles {
        assert!((0.5..=2.0).contains(&p.lifetime), "lifetime {}", p.lifetime);
        assert!((0.1..=0.3).contains(&p.size), "size {}", p.size);
        assert!(
            p.rotation.abs() <= 90f32.to_radians() + 0.1,
            "rotation {}",
            p.rotation
        );
        assert!((0.2..=0.8).contains(&p.color[1]), "green {}", p.color[1]);
        short |= p.lifetime < 1.0;
        long |= p.lifetime > 1.5;
    }
    assert!(
        short && long,
        "the range should actually spread the lifetimes"
    );
}

#[test]
fn old_defaults_keep_the_linear_fade() {
    let e = ParticleEmitterComponent::default();
    assert_eq!(e.shape, EmitShape::Point);
    assert_eq!(e.size_over_life.evaluate(0.7), 1.0, "size stays constant");
    assert_eq!(
        e.color_over_life.evaluate(0.25)[3],
        0.75,
        "alpha fades linearly"
    );
    assert_eq!(e.drag.evaluate(0.5), 0.0);
}
