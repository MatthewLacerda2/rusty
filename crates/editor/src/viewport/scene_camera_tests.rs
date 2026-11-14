//! Unit tests for the Scene view camera's math (#745): orbit keeps the pivot, look
//! keeps the eye, pan and zoom scale with distance, frame fits the bounds.

use super::*;

fn near(a: Vec3, b: Vec3) -> bool {
    a.abs_diff_eq(b, 1e-3)
}

fn pose() -> SceneCamera {
    SceneCamera::from_camera(&Camera::new(Vec3::new(0.0, 5.0, -10.0), 90.0, -20.0))
}

#[test]
fn a_pose_from_a_camera_sees_what_it_sees() {
    let cam = Camera::new(Vec3::new(1.0, 2.0, 3.0), 30.0, -10.0);
    let pose = SceneCamera::from_camera(&cam);
    let out = pose.camera();
    assert!(near(out.position, cam.position));
    assert_eq!((out.yaw, out.pitch), (cam.yaw, cam.pitch));
    assert!(near(
        pose.pivot,
        cam.position + cam.forward() * DEFAULT_DISTANCE
    ));
}

#[test]
fn orbit_turns_around_the_pivot() {
    let mut cam = pose();
    let pivot = cam.pivot;
    cam.orbit(Vec2::new(120.0, -40.0));
    assert!(near(cam.pivot, pivot), "the pivot stays put");
    assert!((cam.position().distance(pivot) - DEFAULT_DISTANCE).abs() < 1e-3);
    assert!(
        (cam.yaw - (90.0 + 30.0)).abs() < 1e-4,
        "drag right yaws right"
    );
    assert!(
        (cam.pitch - (-20.0 + 10.0)).abs() < 1e-4,
        "drag up pitches up"
    );
}

#[test]
fn look_turns_in_place() {
    let mut cam = pose();
    let eye = cam.position();
    cam.look(Vec2::new(-200.0, 60.0));
    assert!(near(cam.position(), eye), "the eye stays put");
    assert!(near(cam.pivot, eye + cam.forward() * cam.distance));
}

#[test]
fn pitch_clamps_short_of_the_poles() {
    let mut cam = pose();
    cam.orbit(Vec2::new(0.0, -10_000.0));
    assert_eq!(cam.pitch, MAX_PITCH);
    cam.look(Vec2::new(0.0, 10_000.0));
    assert_eq!(cam.pitch, -MAX_PITCH);
}

#[test]
fn pan_follows_the_pointer_and_scales_with_distance() {
    let mut cam = pose();
    let before = cam.pivot;
    cam.pan(Vec2::new(100.0, 0.0), 500.0);
    let moved = cam.pivot - before;
    // Drag right: the scene follows the pointer, so the camera slides left.
    assert!(moved.dot(cam.right()) < 0.0);
    assert!(moved.dot(cam.forward()).abs() < 1e-4, "pan never dollies");
    // A full-height drag covers the frustum's height at the pivot depth.
    let mut cam = pose();
    let before = cam.pivot;
    cam.pan(Vec2::new(0.0, 500.0), 500.0);
    let span = 2.0 * DEFAULT_DISTANCE * (DEFAULT_FOV * 0.5).to_radians().tan();
    assert!(((cam.pivot - before).length() - span).abs() < 1e-3);
    // Twice as far back, the same drag covers twice the ground.
    let mut far = pose();
    far.distance *= 2.0;
    let before = far.pivot;
    far.pan(Vec2::new(0.0, 500.0), 500.0);
    assert!(((far.pivot - before).length() - 2.0 * span).abs() < 1e-3);
}

#[test]
fn zoom_scales_the_distance_and_never_sticks() {
    let mut cam = pose();
    cam.zoom(1.0);
    assert!((cam.distance - DEFAULT_DISTANCE * (1.0 - ZOOM_PER_STEP)).abs() < 1e-4);
    cam.zoom(-1.0);
    assert!(
        (cam.distance - DEFAULT_DISTANCE).abs() < 1e-4,
        "out undoes in"
    );
    // Past the minimum the pivot moves ahead, so the eye keeps advancing.
    let eye = cam.position();
    cam.zoom(100.0);
    assert_eq!(cam.distance, MIN_DISTANCE);
    let eye2 = cam.position();
    cam.zoom(100.0);
    assert!((eye2 - eye).dot(cam.forward()) > 0.0);
    assert!((cam.position() - eye2).dot(cam.forward()) >= 0.0);
    cam.zoom(-1000.0);
    assert_eq!(cam.distance, MAX_DISTANCE);
}

#[test]
fn fly_moves_eye_and_pivot_together() {
    let mut cam = pose();
    let (eye, pivot) = (cam.position(), cam.pivot);
    cam.fly(Vec3::new(0.0, 0.0, 1.0), false, 0.5);
    let step = cam.forward() * DEFAULT_FLY_SPEED * 0.5;
    assert!(near(cam.position(), eye + step));
    assert!(near(cam.pivot, pivot + step));
    // Q/E are world up/down whatever the pitch; Shift is faster.
    let mut cam = pose();
    let eye = cam.position();
    cam.fly(Vec3::new(0.0, 1.0, 0.0), true, 1.0);
    assert!(near(
        cam.position(),
        eye + Vec3::Y * DEFAULT_FLY_SPEED * FAST_MULTIPLIER
    ));
    // Diagonals are not faster than straight lines.
    let mut cam = pose();
    let eye = cam.position();
    cam.fly(Vec3::new(1.0, 0.0, 1.0), false, 1.0);
    assert!((cam.position().distance(eye) - DEFAULT_FLY_SPEED).abs() < 1e-3);
    cam.fly(Vec3::ZERO, false, 1.0);
}

#[test]
fn scroll_while_flying_changes_speed_within_range() {
    let mut cam = pose();
    cam.adjust_fly_speed(1.0);
    assert!((cam.fly_speed - DEFAULT_FLY_SPEED * 1.2).abs() < 1e-4);
    cam.adjust_fly_speed(100.0);
    assert_eq!(cam.fly_speed, FLY_SPEED_RANGE.1);
    cam.adjust_fly_speed(-100.0);
    assert_eq!(cam.fly_speed, FLY_SPEED_RANGE.0);
}

#[test]
fn frame_fits_the_bounds_in_the_field_of_view() {
    let mut cam = pose();
    let (yaw, pitch) = (cam.yaw, cam.pitch);
    let center = Vec3::new(4.0, 1.0, -3.0);
    cam.frame(center, 2.0);
    assert!(near(cam.pivot, center));
    assert_eq!(
        (cam.yaw, cam.pitch),
        (yaw, pitch),
        "framing keeps the view direction"
    );
    // The sphere's tangent from the eye sits exactly on the half field of view.
    let half = (DEFAULT_FOV * 0.5).to_radians();
    assert!((2.0 / cam.distance - half.sin()).abs() < 1e-4);
    // A bigger thing frames from farther away; a point still gets a usable distance.
    let near_d = cam.distance;
    cam.frame(center, 8.0);
    assert!(cam.distance > near_d);
    cam.frame(center, 0.0);
    assert!(cam.distance > MIN_DISTANCE);
}
