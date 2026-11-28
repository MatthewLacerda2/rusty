//! `advance` is semi-implicit Euler, whose closed form is x_n = n·dt·v0 + dt²·g·n(n+1)/2.

use glam::{Mat4, Vec3};

use super::*;
use crate::passes::particles::gpu::twin::{advance, birth, lut_at, sprite};
use rusty_components::particle::{Birth, Particle};
use rusty_core::curve::{Curve, Gradient};

fn record(position: Vec3, velocity: Vec3, lifetime: f32) -> GpuParticle {
    let mut particle = Particle::at(position, 1.0, [1.0; 4], lifetime);
    particle.velocity = velocity;
    let birth = Birth {
        particle,
        slot: 0,
        step: 0,
        time: 0.0,
        serial: 0,
        dead: false,
    };
    GpuParticle::born(&birth)
}

#[test]
fn the_records_match_the_shader_layout() {
    assert_eq!(std::mem::size_of::<GpuParticle>(), 64);
    assert_eq!(std::mem::size_of::<GpuBirth>(), 80);
    assert_eq!(std::mem::size_of::<GpuEmitter>(), 176);
}

#[test]
fn the_table_samples_every_curve_end_to_end() {
    let emitter = ParticleEmitterComponent {
        size_over_life: Curve::linear(1.0, 0.0),
        drag: Curve::constant(0.5),
        color_over_life: Gradient::fade_out(),
        ..Default::default()
    };
    let lut = bake_lut(&emitter);
    assert_eq!(lut.len(), 2 * LUT_SIZE);
    assert_eq!(lut_at(&lut, 0, 0.0).x, 1.0);
    assert!(lut_at(&lut, 0, 1.0).x.abs() < 1e-6);
    assert!(
        (lut_at(&lut, 0, 0.5).x - 0.5).abs() < 1e-3,
        "linear between samples"
    );
    assert_eq!(lut_at(&lut, 0, 0.3).y, 0.5);
    assert_eq!(
        lut_at(&lut, 1, 0.0).w,
        emitter.color_over_life.evaluate(0.0)[3]
    );
    assert_eq!(lut_at(&lut, 1, 2.0), lut_at(&lut, 1, 1.0), "t clamps");
}

#[test]
fn a_step_under_gravity_lands_where_the_closed_form_says() {
    let lut = bake_lut(&ParticleEmitterComponent {
        drag: Curve::constant(0.0),
        ..Default::default()
    });
    let (v0, g, dt, n) = (
        Vec3::new(1.0, 2.0, 0.0),
        Vec3::new(0.0, -9.81, 0.0),
        1.0 / 60.0,
        30,
    );
    let mut p = record(Vec3::ZERO, v0, 10.0);
    advance(&mut p, (n, 1), g, &lut, (&[dt; 30], 0.0));
    let nf = n as f32;
    let want = v0 * nf * dt + g * dt * dt * nf * (nf + 1.0) / 2.0;
    assert!(
        (Vec3::from(p.position) - want).length() < 1e-4,
        "{:?} vs {want:?}",
        p.position
    );
    assert_eq!(p.next, n + 1);
    assert!((p.age - nf * dt).abs() < 1e-5);
}

#[test]
fn a_particle_dies_on_the_step_its_age_reaches_its_lifetime_and_stays_dead() {
    let lut = bake_lut(&ParticleEmitterComponent::default());
    let mut p = record(Vec3::ZERO, Vec3::ZERO, 0.05);
    advance(&mut p, (2, 1), Vec3::ZERO, &lut, (&[1.0 / 60.0; 2], 0.0));
    assert!(p.alive());
    advance(&mut p, (10, 3), Vec3::ZERO, &lut, (&[1.0 / 60.0; 8], 0.0));
    assert!(!p.alive(), "0.05 s is three steps");
    let emitter = GpuEmitter::new(
        &ParticleEmitterComponent::default(),
        Mat4::IDENTITY,
        [0.0; 4],
    );
    assert!(sprite(&p, &emitter, &lut).is_none());
    let mut dead = GpuParticle::EMPTY;
    advance(&mut dead, (5, 1), Vec3::Y, &lut, (&[1.0; 5], 0.0));
    assert_eq!(dead, GpuParticle::EMPTY);
}

#[test]
fn a_local_sprite_draws_under_the_emitter_frame_with_its_curves() {
    let source = ParticleEmitterComponent {
        size_over_life: Curve::constant(2.0),
        gravity: Vec3::new(0.0, -1.0, 0.0),
        ..Default::default()
    };
    let lut = bake_lut(&source);
    let frame = Mat4::from_scale_rotation_translation(
        Vec3::splat(2.0),
        glam::Quat::IDENTITY,
        Vec3::new(3.0, 0.0, 0.0),
    );
    let e = GpuEmitter::new(&source, frame, [0.0; 4]);
    assert_eq!(
        e.gravity[1], -0.5,
        "a world acceleration in the emitter's own space"
    );
    let p = record(Vec3::X, Vec3::X, 1.0);
    let s = sprite(&p, &e, &lut).expect("live");
    assert_eq!(s.center, [5.0, 0.0, 0.0]);
    assert_eq!(s.size, 2.0, "the size curve, never scaled by the frame");
    assert_eq!(s.stretch[..3], [1.0, 0.0, 0.0]);
}

#[test]
fn a_dead_birth_uploads_as_an_empty_slot() {
    let birth = Birth {
        particle: Particle::at(Vec3::ONE, 1.0, [1.0; 4], 1.0),
        slot: 3,
        step: 7,
        time: 0.0,
        serial: 0,
        dead: true,
    };
    assert_eq!(GpuParticle::born(&birth), GpuParticle::EMPTY);
    let live = GpuParticle::born(&Birth {
        dead: false,
        ..birth
    });
    assert_eq!(live.next, 8, "its first step is the one after its birth's");
}

#[test]
fn steps_older_than_the_window_jump_in_closed_form_where_stepping_would_land() {
    let mut lut_source = ParticleEmitterComponent {
        drag: Curve::constant(0.0),
        ..Default::default()
    };
    let (v0, g, dt) = (Vec3::new(1.0, 2.0, 0.0), Vec3::new(0.0, -9.81, 0.0), 0.01);
    for drag in [0.0, 0.5, 200.0] {
        lut_source.drag = Curve::constant(drag);
        let lut = bake_lut(&lut_source);
        let (mut stepped, mut jumped) = (record(Vec3::ZERO, v0, 1e6), record(Vec3::ZERO, v0, 1e6));
        advance(&mut stepped, (1000, 1), g, &lut, (&[dt; 1000], 0.0));
        advance(&mut jumped, (1000, 991), g, &lut, (&[dt; 10], 990.0 * dt));
        let (a, b) = (Vec3::from(stepped.position), Vec3::from(jumped.position));
        assert!(
            (a - b).length() < 1e-2 * a.length().max(1.0),
            "drag {drag}: {a} vs {b}"
        );
        assert!((stepped.age - jumped.age).abs() < 1e-3);
        assert_eq!(jumped.next, 1001);
    }
    let mut short = record(Vec3::ZERO, v0, 0.5);
    let lut = bake_lut(&ParticleEmitterComponent::default());
    advance(&mut short, (1000, 991), g, &lut, (&[dt; 10], 990.0 * dt));
    assert!(
        !short.alive(),
        "a particle dead inside the jump is dead after it"
    );
}

#[test]
fn a_jump_over_a_pause_ages_by_the_time_that_passed_not_the_windows_dt() {
    let lut = bake_lut(&ParticleEmitterComponent::default());
    let dt = 1.0 / 60.0;
    let paused = [0.0; 256];
    let upload = |lifetime: f32| GpuBirth {
        particle: record(Vec3::ZERO, Vec3::X, lifetime),
        slot: 0,
        jumped: 300.0 * dt,
        pad: [0; 2],
    };
    let mut long = birth(&upload(6.0), 345, Vec3::ZERO, &lut);
    advance(&mut long, (600, 345), Vec3::ZERO, &lut, (&paused, 0.0));
    assert!(long.alive());
    assert!(
        (long.age - 5.0).abs() < 1e-4,
        "aged 5 s, not 0: {}",
        long.age
    );
    assert!(
        (long.position[0] - 5.0).abs() < 1e-3,
        "moved 5 s: {:?}",
        long.position
    );
    let mut short = birth(&upload(4.0), 345, Vec3::ZERO, &lut);
    advance(&mut short, (600, 345), Vec3::ZERO, &lut, (&paused, 0.0));
    assert!(!short.alive(), "a 4 s life ended inside the pause's jump");
}
