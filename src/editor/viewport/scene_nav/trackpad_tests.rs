//! Unit tests for navigating without a mouse (#1344): trackpad swipe and pinch, and
//! the arrow keys.

use super::*;

#[test]
fn a_swipe_looks_and_a_shift_swipe_pans() {
    let start = SceneCamera::default();
    let mut look = start;
    let swipe = NavFrame {
        swipe: Vec2::new(40.0, -20.0),
        viewport_height: 400.0,
        ..Default::default()
    };
    apply(&mut look, &swipe);
    assert!(look.yaw > start.yaw, "swipe right turns right");
    assert!(look.pitch > start.pitch, "swipe up looks up");
    assert!(
        look.position().abs_diff_eq(start.position(), 1e-4),
        "eye stays"
    );
    assert_eq!(look.distance, start.distance, "a swipe never zooms");

    let mut pan = start;
    let shifted = NavFrame {
        swipe_pan: Vec2::new(40.0, 0.0),
        viewport_height: 400.0,
        ..Default::default()
    };
    apply(&mut pan, &shifted);
    assert_eq!((pan.yaw, pan.pitch), (start.yaw, start.pitch));
    assert!((pan.pivot - start.pivot).dot(start.right()) < 0.0);
}

#[test]
fn a_still_frame_leaves_the_camera_alone() {
    let mut cam = SceneCamera::default();
    let start = cam;
    apply(&mut cam, &NavFrame::default());
    assert_eq!(cam, start);
}

#[test]
fn pinch_spread_zooms_in_and_doubling_halves_the_distance() {
    assert_eq!(pinch_steps(1.0), 0.0);
    assert_eq!(pinch_steps(0.0), 0.0);
    let mut cam = SceneCamera::default();
    let distance = cam.distance;
    let pinch = NavFrame {
        pinch: pinch_steps(2.0),
        ..Default::default()
    };
    apply(&mut cam, &pinch);
    assert!((cam.distance - distance * 0.5).abs() < 1e-3);
}

#[test]
fn arrow_keys_turn_at_a_fixed_rate_and_swiping_while_looking_sets_fly_speed() {
    let start = SceneCamera::default();
    let mut cam = start;
    let arrows = NavFrame {
        turn: Vec2::new(1.0, -1.0),
        dt: 0.5,
        ..Default::default()
    };
    apply(&mut cam, &arrows);
    assert!(
        (cam.yaw - start.yaw - 45.0).abs() < 1e-3,
        "right arrow, 90°/s"
    );
    assert!(cam.pitch > start.pitch, "up arrow looks up");

    let mut cam = start;
    let looking = NavFrame {
        gesture: Some(Gesture::Look),
        swipe: Vec2::new(0.0, POINTS_PER_STEP),
        ..Default::default()
    };
    apply(&mut cam, &looking);
    assert!(cam.fly_speed > start.fly_speed);
    assert_eq!((cam.yaw, cam.pitch), (start.yaw, start.pitch));
}
