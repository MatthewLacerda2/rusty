//! Instance building for the sprite pass (#440): modes, flipbook frames, sorting.

use glam::Vec3;

use super::*;
use crate::components::particle::Particle;
use crate::components::Flipbook;

const EYE: Eye = Eye {
    pos: Vec3::ZERO,
    fwd: Vec3::NEG_Z,
};

fn emitter_with(zs: &[f32]) -> ParticleEmitterComponent {
    let mut e = ParticleEmitterComponent::default();
    for &z in zs {
        e.runtime
            .particles
            .push(Particle::at(Vec3::new(0.0, 0.0, z), 1.0, [1.0; 4], 10.0));
    }
    e
}

#[test]
fn alpha_particles_sort_back_to_front_additive_keep_order() {
    let e = emitter_with(&[-2.0, -8.0, -5.0]);
    let (sorted, depth) = emitter_instances(&e, [0.0; 4], EYE);
    let zs: Vec<f32> = sorted.iter().map(|i| i.center[2]).collect();
    assert_eq!(zs, [-8.0, -5.0, -2.0]);
    assert!((depth - 5.0).abs() < 1e-5, "centroid depth {depth}");

    let mut add = emitter_with(&[-2.0, -8.0, -5.0]);
    add.blend = ParticleBlend::Additive;
    let (kept, _) = emitter_instances(&add, [0.0; 4], EYE);
    let zs: Vec<f32> = kept.iter().map(|i| i.center[2]).collect();
    assert_eq!(zs, [-2.0, -8.0, -5.0]);
}

#[test]
fn stretched_carries_velocity_axis_and_length() {
    let mut e = emitter_with(&[-3.0]);
    e.render.mode = ParticleRenderMode::Stretched;
    e.render.length_scale = 2.0;
    e.render.speed_scale = 0.5;
    e.runtime.particles[0].velocity = Vec3::new(4.0, 0.0, 0.0);
    let (inst, _) = emitter_instances(&e, [0.0; 4], EYE);
    assert_eq!(inst[0].mode, 1);
    // Default size curve is flat, so size stays 1: length = 1·2 + 4·0.5.
    assert_eq!(inst[0].stretch, [1.0, 0.0, 0.0, 4.0]);
}

#[test]
fn flipbook_frame_advances_over_life_from_its_start() {
    let book = Flipbook {
        columns: 2,
        rows: 2,
        cycles: 1.0,
        random_start: false,
    };
    let mut p = Particle::at(Vec3::ZERO, 1.0, [1.0; 4], 4.0);
    let frames: Vec<u32> = [0.0, 1.0, 2.5, 3.9, 4.0]
        .into_iter()
        .map(|age| {
            p.age = age;
            book.frame_of(&p)
        })
        .collect();
    assert_eq!(frames, [0, 1, 2, 3, 3], "last frame holds at death");
    p.start_frame = 3;
    p.age = 1.0;
    assert_eq!(book.frame_of(&p), 0, "wraps past the last frame");
}

#[test]
fn lit_flag_reaches_every_instance() {
    let mut e = emitter_with(&[-1.0, -2.0]);
    e.render.soft_distance = 0.75;
    let (inst, _) = emitter_instances(&e, [0.1, 0.2, 0.3, 2.0], EYE);
    assert!(inst.iter().all(|i| i.light == [0.1, 0.2, 0.3, 2.0]));
    assert!(inst.iter().all(|i| i.sheet[3] == 0.75));
}

/// A lit emitter under a probe of a uniform sky of radiance L reads L, the matte
/// response, not the irradiance πL (#807): the same scale as the flat ambient.
#[test]
fn a_probe_lights_a_puff_by_the_sky_radiance_not_its_irradiance() {
    let mut scene = Scene::new();
    let probe = scene.probes.add_probe(Vec3::ZERO);
    // A uniform radiance L projects onto the DC band alone: c0 = L · Y00 · 4π.
    let l = 0.5;
    let c0 = l * 0.282_094_8 * 4.0 * std::f32::consts::PI;
    scene.probes.probes[probe].sh.coeffs[0] = [c0; 3];
    let mut emitter = ParticleEmitterComponent::default();
    emitter.render.lit = true;
    let light = emitter_light(&scene, &emitter, Vec3::ZERO);
    assert!((light[0] - l).abs() < 1e-5, "probe ambient {light:?}");
    assert_eq!(light[3], 2.0);
}
