//! Unit tests for the arrow keys (#1912): Unity's Scene-view move, at the fly speed.

use super::*;

fn arrows(arrows: Vec2, fast: bool) -> NavFrame {
    NavFrame {
        arrows,
        fast,
        dt: 0.5,
        ..Default::default()
    }
}

/// Where `frame` moves a default camera's eye, and the camera after it.
fn moved(frame: &NavFrame) -> (Vec3, SceneCamera) {
    let mut cam = SceneCamera::default();
    let eye = cam.position();
    apply(&mut cam, frame);
    (cam.position() - eye, cam)
}

#[test]
fn up_moves_along_forward_and_left_strafes_left_at_the_fly_speed() {
    let start = SceneCamera::default();
    let step = start.fly_speed * 0.5;
    let (up, cam) = moved(&arrows(Vec2::Y, false));
    assert!(up.abs_diff_eq(start.forward() * step, 1e-4), "{up}");
    assert_eq!((cam.yaw, cam.pitch), (start.yaw, start.pitch), "no turn");
    assert_eq!(cam.distance, start.distance);
    let (down, _) = moved(&arrows(Vec2::NEG_Y, false));
    assert!(down.abs_diff_eq(-start.forward() * step, 1e-4));
    let (left, cam) = moved(&arrows(Vec2::NEG_X, false));
    assert!(left.abs_diff_eq(-start.right() * step, 1e-4), "{left}");
    assert_eq!((cam.yaw, cam.pitch), (start.yaw, start.pitch), "no turn");
}

#[test]
fn shift_moves_further_in_the_same_dt() {
    let (slow, _) = moved(&arrows(Vec2::Y, false));
    let (fast, _) = moved(&arrows(Vec2::Y, true));
    assert!(fast.length() > slow.length() * 2.0);
    assert!(fast.normalize().abs_diff_eq(slow.normalize(), 1e-4));
}

#[test]
fn no_arrow_leaves_the_camera_still() {
    let (offset, cam) = moved(&arrows(Vec2::ZERO, true));
    assert_eq!(offset, Vec3::ZERO);
    assert_eq!(cam, SceneCamera::default());
}

#[test]
fn the_scroll_adjusted_fly_speed_carries_over() {
    let mut cam = SceneCamera::default();
    cam.adjust_fly_speed(4.0);
    let eye = cam.position();
    apply(&mut cam, &arrows(Vec2::Y, false));
    assert!(((cam.position() - eye).length() - cam.fly_speed * 0.5).abs() < 1e-4);
}

#[test]
fn arrows_add_to_wasd_while_looking_in_one_fly_step() {
    let start = SceneCamera::default();
    let fly = |fly, arrows| NavFrame {
        gesture: Some(Gesture::Look),
        fly,
        arrows,
        dt: 0.5,
        ..Default::default()
    };
    // Up with W is still one fly step forward, never two.
    let (both, _) = moved(&fly(Vec3::Z, Vec2::Y));
    let (w, _) = moved(&fly(Vec3::Z, Vec2::ZERO));
    assert!(both.abs_diff_eq(w, 1e-4), "{both} vs {w}");
    // Up alone while looking still moves (no stall), as far as W.
    let (up, _) = moved(&fly(Vec3::ZERO, Vec2::Y));
    assert!(up.abs_diff_eq(w, 1e-4));
    // Right with W heads diagonally: the axes add.
    let (diag, _) = moved(&fly(Vec3::Z, Vec2::X));
    assert!(diag.dot(start.right()) > 0.0 && diag.dot(start.forward()) > 0.0);
}

#[test]
fn a_held_arrow_keeps_asking_for_frames() {
    assert!(arrows(Vec2::NEG_X, false).keeps_moving());
    assert!(!arrows(Vec2::ZERO, false).keeps_moving());
}
