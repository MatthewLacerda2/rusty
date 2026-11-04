//! TAA's ghosts on lavapipe (#1702): a moving cube and a moving sprite particle
//! (which writes no motion) leave no trail, a cut leaves no ghost of the last scene,
//! and a viewmodel under a turning camera stays as sharp as without TAA.

use glam::{Quat, Vec3};

use super::gpu_fixture::{
    cube, diff, eye, frame, fresh_frame, pixel, project, renderer, settle, taa_scene, view,
};
use crate::components::particle::{Particle, ParticleEmitterComponent};
use crate::components::CameraComponent;
use crate::render::motion::gpu_fixture::{H, W};
use crate::scene::{ClearFlags, Scene};

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
    let path = std::env::temp_dir().join("rusty_1702_taa_white.png");
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

#[test]
fn gpu_a_scene_load_leaves_no_ghost_of_the_last_scene() {
    let Some(mut r) = renderer() else { return };
    let cam = eye(0.0);
    let (mut before, _) = taa_scene(true, true);
    cube(&mut before, Vec3::new(0.0, 0.0, -5.0));
    let mut v = view(&r);
    settle(&mut r, &mut v, &before, &cam, 12);
    // Another scene (another id): a cut, as Play, Stop or a camera switch are.
    let (after, _) = taa_scene(true, true);
    let first = frame(&mut r, &mut v, &after, &cam);
    let fresh = fresh_frame(&mut r, &after, &cam);
    let worst = (0..W * H)
        .map(|i| diff(&first, &fresh, (i % W, i / W)))
        .max()
        .unwrap();
    assert!(
        worst <= TRAIL,
        "the first frame after the cut kept {worst} of the last scene"
    );
}

/// The fixture scene, a cube 6 m out to the right, and a `DepthOnly` overlay camera
/// holding a gun cube 3 m ahead (the motion-blur viewmodel stack). Returns the
/// scene and the overlay camera.
fn viewmodel(taa: bool) -> (Scene, u32) {
    let (mut scene, _) = taa_scene(taa, false);
    cube(&mut scene, Vec3::new(2.0, 0.0, -6.0));
    let overlay = scene.add_entity("ViewmodelCamera".to_string());
    let lens = CameraComponent {
        render_order: 1,
        clear_flags: ClearFlags::DepthOnly,
        ..Default::default()
    };
    scene.world.set_camera(overlay, Some(lens));
    let gun = cube(&mut scene, Vec3::new(0.0, 0.0, -3.0));
    scene.set_parent(gun, Some(overlay)).unwrap();
    (scene, overlay)
}

#[test]
fn gpu_a_viewmodel_does_not_swim_against_the_turning_world() {
    let Some(mut r) = renderer() else { return };
    let (mut scene, overlay) = viewmodel(true);
    let mut v = view(&r);
    let mut last = Vec::new();
    for step in 0..8 {
        let yaw = step as f32 * 3.0;
        scene.world.transform_mut(overlay).unwrap().rotation =
            Quat::from_rotation_y(-yaw.to_radians());
        last = frame(&mut r, &mut v, &scene, &eye(yaw));
    }
    let (mut plain, overlay) = viewmodel(false);
    plain.world.transform_mut(overlay).unwrap().rotation =
        Quat::from_rotation_y(-21f32.to_radians());
    let sharp = fresh_frame(&mut r, &plain, &eye(21.0));
    let gun = (29..=35).flat_map(|x| (21..=27).map(move |y| (x, y)));
    let worst = gun.map(|p| diff(&last, &sharp, p)).max().unwrap();
    eprintln!("[taa] the gun under a turning camera differs from no TAA by {worst}");
    assert!(worst <= 6, "the gun swam or smeared: {worst}");
}
