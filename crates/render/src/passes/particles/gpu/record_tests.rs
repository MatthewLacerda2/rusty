//! The GPU records on the CPU (#2170): the lookup-table bake, the step mirror
//! against the closed form, the sprite, and which adapters simulate.

use glam::{Mat4, Vec3};

use super::*;
use rusty_components::particle::{Birth, Particle};
use rusty_core::curve::{Curve, Gradient};

fn record(position: Vec3, velocity: Vec3, lifetime: f32) -> GpuParticle {
    let mut particle = Particle::at(position, 1.0, [1.0; 4], lifetime);
    particle.velocity = velocity;
    let birth = Birth {
        particle,
        slot: 0,
        step: 0,
        serial: 0,
        dead: false,
    };
    GpuParticle::born(&birth)
}

#[test]
fn the_records_match_the_shader_layout() {
    assert_eq!(std::mem::size_of::<GpuParticle>(), 64);
    assert_eq!(std::mem::size_of::<GpuBirth>(), 80);
    assert_eq!(std::mem::size_of::<GpuEmitter>(), 160);
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
    advance(&mut p, n, g, &lut, |_| dt);
    // Semi-implicit Euler: x_n = n·dt·v0 + dt²·g·n(n+1)/2.
    let nf = n as f32;
    let want = v0 * nf * dt + g * dt * dt * nf * (nf + 1.0) / 2.0;
    assert!(
        (Vec3::from(p.position) - want).length() < 1e-4,
        "{:?} vs {want:?}",
        p.position
    );
    assert_eq!(p.done, n);
    assert!((p.age - nf * dt).abs() < 1e-5);
}

#[test]
fn a_particle_dies_on_the_step_its_age_reaches_its_lifetime_and_stays_dead() {
    let lut = bake_lut(&ParticleEmitterComponent::default());
    let mut p = record(Vec3::ZERO, Vec3::ZERO, 0.05);
    advance(&mut p, 2, Vec3::ZERO, &lut, |_| 1.0 / 60.0);
    assert!(p.alive());
    advance(&mut p, 10, Vec3::ZERO, &lut, |_| 1.0 / 60.0);
    assert!(!p.alive(), "0.05 s is three steps");
    let emitter = GpuEmitter::new(
        &ParticleEmitterComponent::default(),
        Mat4::IDENTITY,
        [0.0; 4],
    );
    assert!(sprite(&p, &emitter, &lut).is_none());
    let mut dead = GpuParticle::EMPTY;
    advance(&mut dead, 5, Vec3::Y, &lut, |_| 1.0);
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
        serial: 0,
        dead: true,
    };
    assert_eq!(GpuParticle::born(&birth), GpuParticle::EMPTY);
    let live = GpuParticle::born(&Birth {
        dead: false,
        ..birth
    });
    assert_eq!(live.done, 7);
}

#[test]
fn only_an_adapter_with_compute_and_vertex_storage_simulates() {
    use wgpu::DownlevelFlags as F;
    assert!(crate::passes::particles::gpu::supported(F::all()));
    assert!(!crate::passes::particles::gpu::supported(
        F::all() - F::COMPUTE_SHADERS
    ));
    assert!(!crate::passes::particles::gpu::supported(
        F::all() - F::VERTEX_STORAGE
    ));
}
