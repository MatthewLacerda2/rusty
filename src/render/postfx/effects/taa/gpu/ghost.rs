//! TAA's ghosts on lavapipe (#1702): a moving cube and a moving sprite particle
//! (which writes no motion) leave no trail. A cut: `cut`; a viewmodel:
//! `viewmodel`.

use glam::Vec3;

use super::fixture::{
    cube, diff, eye, frame, fresh_frame, pixel, project, renderer, settle, taa_scene, view,
};
use crate::components::particle::{Particle, ParticleEmitterComponent};

/// Frames after the move a trail may take to fade, and how far off the unjittered
/// sky a pixel may be then (the jittered sky's own gradient moves a sub-pixel).
const AFTER: u32 = 4;
const TRAIL: u8 = 4;

/// Pixel at world point `p`'s projection.
fn at(p: Vec3) -> (u32, u32) {
    pixel(project(&eye(0.0), p))
}

#[test]
fn gpu_a_moving_cube_leaves_no_trail() {
    let Some(mut r) = renderer() else { return };
    let cam = eye(0.0);
    let (mut scene, _) = taa_scene(true, true);
    let id = cube(&mut scene, Vec3::new(-0.6, 0.0, -5.0));
    let mut v = view(&r);
    settle(&mut r, &mut v, &scene, &cam, 12);
    scene.world.transform_mut(id).unwrap().position = Vec3::new(0.6, 0.0, -5.0);
    let first = frame(&mut r, &mut v, &scene, &cam);
    let last = settle(&mut r, &mut v, &scene, &cam, AFTER);
    let (sky, _) = taa_scene(false, false);
    let empty = fresh_frame(&mut r, &sky, &cam);
    let was = at(Vec3::new(-0.8, 0.0, -4.5));
    eprintln!(
        "[taa] where the cube was: {} off the sky the first frame, {} after {AFTER}",
        diff(&first, &empty, was),
        diff(&last, &empty, was)
    );
    assert!(diff(&last, &empty, was) <= TRAIL, "the cube left a trail");
}

/// A white sprite in the temp dir, so a particle shows its tint.
fn white_sprite() -> String {
    let path = crate::test_temp::dir().join("rusty_1702_taa_white.png");
    image::RgbaImage::from_pixel(4, 4, image::Rgba([255; 4]))
        .save(&path)
        .unwrap();
    path.to_string_lossy().into_owned()
}

#[test]
fn gpu_a_moving_sprite_particle_leaves_no_trail() {
    let Some(mut r) = renderer() else { return };
    let cam = eye(0.0);
    let (mut scene, _) = taa_scene(true, true);
    let id = scene.add_entity("Emitter".to_string());
    let mut emitter = ParticleEmitterComponent {
        texture: Some(white_sprite().into()),
        ..Default::default()
    };
    let red = [1.0, 0.1, 0.1, 1.0];
    emitter.runtime.particles = vec![Particle::at(Vec3::new(-0.6, 0.0, -5.0), 0.8, red, 99.0)];
    scene.world.set_particles(id, Some(emitter));
    let mut v = view(&r);
    let held = settle(&mut r, &mut v, &scene, &cam, 12);
    let was = at(Vec3::new(-0.6, 0.0, -5.0));
    let mut p = scene.world.particles_mut(id).unwrap();
    p.runtime.particles[0].position = Vec3::new(0.6, 0.0, -5.0);
    drop(p);
    let last = settle(&mut r, &mut v, &scene, &cam, AFTER);
    let (sky, _) = taa_scene(false, false);
    let empty = fresh_frame(&mut r, &sky, &cam);
    assert!(
        diff(&held, &empty, was) > 40,
        "the particle drew where it was"
    );
    let trail = diff(&last, &empty, was);
    eprintln!("[taa] where the particle was: {trail} off the sky after {AFTER} frames");
    assert!(trail <= TRAIL, "the particle left a trail: {trail}");
}
