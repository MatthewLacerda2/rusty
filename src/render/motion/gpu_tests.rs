//! The velocity target on lavapipe (#1435): an object moving past a still camera, a
//! still scene under a turning camera, and the gate that keeps it off.

use glam::Vec3;

use super::gpu_fixture::{
    cube, eye, pixel_centre, project, render, renderer, scene, unproject, view, H, W,
};

#[test]
fn gpu_a_cube_moving_past_a_still_camera_writes_velocity_on_its_pixels_only() {
    let Some(mut r) = renderer() else { return };
    let mut view = view(&r);
    let (mut scene, _) = scene(true);
    let id = cube(&mut scene, Vec3::new(-0.5, 0.0, -5.0));
    let cam = eye(0.0);
    let first = render(&mut r, &mut view, &scene, &cam).expect("motion blur on");
    assert!(
        first.centre().length() < 1e-4,
        "a first draw has no history"
    );
    assert_eq!(r.frame_counters.velocity_passes, 1);

    let to = Vec3::new(0.5, 0.0, -5.0);
    scene.world.transform_mut(id).unwrap().position = to;
    let moved = render(&mut r, &mut view, &scene, &cam).unwrap();
    // The front face's centre moved by exactly its projected step.
    let face = to + Vec3::Z * 0.5;
    let expected = project(&cam, face) - project(&cam, face - Vec3::X);
    let got = moved.at_ndc(project(&cam, face));
    eprintln!("[motion] moving cube: got {got}, expected {expected}");
    assert!((got - expected).length() < 0.01, "{got} vs {expected}");
    assert!(got.x > 0.1, "it moved right: {got}");
    // The sky behind it, and the camera, did not move.
    assert!(moved.at(1, 1).length() < 1e-4, "{}", moved.at(1, 1));
    assert!(moved.at(W - 2, H - 2).length() < 1e-4);
}

#[test]
fn gpu_a_still_scene_under_a_turning_camera_matches_camera_reprojection() {
    let Some(mut r) = renderer() else { return };
    let mut view = view(&r);
    let (mut scene, _) = scene(true);
    cube(&mut scene, Vec3::new(0.0, 0.0, -5.0));
    let (before, after) = (eye(0.0), eye(3.0));
    render(&mut r, &mut view, &scene, &before).unwrap();
    let turned = render(&mut r, &mut view, &scene, &after).unwrap();

    // A point on the cube: its camera-only reprojection.
    let face = Vec3::new(0.0, 0.0, -4.5);
    let expected = project(&after, face) - project(&before, face);
    let got = turned.at_ndc(project(&after, face));
    eprintln!("[motion] turning camera on the cube: got {got}, expected {expected}");
    assert!((got - expected).length() < 0.01, "{got} vs {expected}");
    assert!(expected.length() > 0.05, "the turn moves the image");

    // The sky at a corner pixel: its direction, reprojected through the rotation.
    let corner = pixel_centre((3, 3));
    let dir = unproject(&after, corner) * 100.0;
    let sky = corner - project(&before, dir);
    let got = turned.at(3, 3);
    eprintln!("[motion] turning camera on the sky: got {got}, expected {sky}");
    assert!((got - sky).length() < 0.005, "{got} vs {sky}");
}

#[test]
fn gpu_no_motion_blur_writes_no_velocity_target() {
    let Some(mut r) = renderer() else { return };
    let mut view = view(&r);
    let (mut blurred, _) = scene(true);
    cube(&mut blurred, Vec3::new(0.0, 0.0, -5.0));
    let (mut plain, _) = scene(false);
    cube(&mut plain, Vec3::new(0.0, 0.0, -5.0));
    let cam = eye(0.0);
    assert!(render(&mut r, &mut view, &plain, &cam).is_none());
    assert_eq!(r.frame_counters.velocity_passes, 0);
    r.quality = crate::core::settings::quality::QualityPreset::Low;
    assert!(
        render(&mut r, &mut view, &blurred, &cam).is_none(),
        "Low pays nothing"
    );
    assert_eq!(r.frame_counters.velocity_passes, 0);
}

#[test]
fn gpu_the_player_option_off_writes_no_velocity_target() {
    use crate::core::settings::graphics_options::GraphicsOption;
    let Some(mut r) = renderer() else { return };
    let mut view = view(&r);
    let (mut scene, _) = scene(true);
    cube(&mut scene, Vec3::new(0.0, 0.0, -5.0));
    r.graphics_options.set(GraphicsOption::MotionBlur, false);
    assert!(
        render(&mut r, &mut view, &scene, &eye(0.0)).is_none(),
        "the player turned motion blur off (#1438): no velocity pass"
    );
    assert_eq!(r.frame_counters.velocity_passes, 0);
}
