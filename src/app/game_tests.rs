//! Unit tests for `GameWorld` (game.rs): editor-mode fly camera, play/stop
//! transitions, the scaled-dt threading, and the snapshot capture/restore boundary.
//! All drive the real `tick`, honouring the determinism contract (fixed dt, no
//! wall-clock, no RNG).

use super::*;
use crate::scene::Scene;
use std::cell::RefCell;
use std::rc::Rc;

/// A bare play-mode world over `scene`; nav grid spans the demo bounds.
fn world_with(scene: Scene) -> GameWorld {
    let scene = Rc::new(RefCell::new(scene));
    let input = Rc::new(RefCell::new(InputState::new()));
    let nav = Rc::new(RefCell::new(NavigationGraph::new(
        -20.0, 20.0, -20.0, 20.0, 1.0,
    )));
    let console = Rc::new(RefCell::new(ConsoleLogs::new()));
    GameWorld::new(scene, input, nav, console)
}

fn empty_world() -> GameWorld {
    world_with(Scene::new())
}

const DT: f32 = 1.0 / 60.0;

#[test]
fn accessors_expose_shared_handles() {
    let gw = empty_world();
    // Each accessor returns the very cell the resources hold (same allocation).
    assert!(Rc::ptr_eq(gw.scene(), &gw.world.scene));
    assert!(Rc::ptr_eq(gw.input(), &gw.resources.input));
    assert!(Rc::ptr_eq(gw.nav(), &gw.resources.nav));
    assert!(Rc::ptr_eq(gw.console(), &gw.resources.console));
    assert!(Rc::ptr_eq(gw.camera(), &gw.resources.camera));
    assert!(Rc::ptr_eq(gw.time(), &gw.resources.time));
    assert!(!gw.is_playing());
    assert_eq!(gw.play_frame(), 0);
    assert!(gw.pathfinding_points().is_empty());
    let _ = gw.script_manager();
}

#[test]
fn set_playing_flips_state() {
    let mut gw = empty_world();
    assert!(!gw.is_playing());
    gw.set_playing(true);
    assert!(gw.is_playing());
    gw.set_playing(false);
    assert!(!gw.is_playing());
}

#[test]
fn editor_fly_translates_forward_on_w() {
    let mut gw = empty_world();
    let start = gw.camera().borrow().position;
    let fwd = gw.camera().borrow().forward();
    gw.input().borrow_mut().set_key_state("W", true);
    gw.tick(DT);
    let moved = gw.camera().borrow().position - start;
    // W moves +forward by 10 units/sec * dt.
    let expected = fwd * 10.0 * DT;
    assert!(
        moved.abs_diff_eq(expected, 1e-5),
        "moved {moved:?} expected {expected:?}"
    );
}

#[test]
fn editor_fly_s_and_w_cancel() {
    let mut gw = empty_world();
    let start = gw.camera().borrow().position;
    gw.input().borrow_mut().set_key_state("W", true);
    gw.input().borrow_mut().set_key_state("S", true);
    gw.tick(DT);
    // Opposing keys net zero movement (no normalize of a zero vector either).
    assert!(gw.camera().borrow().position.abs_diff_eq(start, 1e-6));
}

#[test]
fn editor_fly_strafe_uses_right_axis() {
    let mut gw = empty_world();
    let right = gw.camera().borrow().right();
    let start = gw.camera().borrow().position;
    gw.input().borrow_mut().set_key_state("D", true);
    gw.tick(DT);
    let moved = gw.camera().borrow().position - start;
    assert!(moved.abs_diff_eq(right * 10.0 * DT, 1e-5));
    // A strafes the opposite way.
    let mut gw = empty_world();
    let start = gw.camera().borrow().position;
    gw.input().borrow_mut().set_key_state("A", true);
    gw.tick(DT);
    let moved = gw.camera().borrow().position - start;
    assert!(moved.dot(right) < 0.0);
}

#[test]
fn editor_fly_yaw_and_pitch_track_arrow_keys() {
    let mut gw = empty_world();
    let (yaw0, pitch0) = {
        let c = gw.camera().borrow();
        (c.yaw, c.pitch)
    };
    gw.input().borrow_mut().set_key_state("RIGHT", true);
    gw.input().borrow_mut().set_key_state("UP", true);
    gw.tick(DT);
    let c = gw.camera().borrow();
    // 90 deg/sec each; RIGHT increases yaw, UP increases pitch.
    assert!((c.yaw - (yaw0 + 90.0 * DT)).abs() < 1e-4);
    assert!((c.pitch - (pitch0 + 90.0 * DT)).abs() < 1e-4);
}

#[test]
fn editor_fly_left_and_down_decrease_angles() {
    let mut gw = empty_world();
    let (yaw0, pitch0) = {
        let c = gw.camera().borrow();
        (c.yaw, c.pitch)
    };
    gw.input().borrow_mut().set_key_state("LEFT", true);
    gw.input().borrow_mut().set_key_state("DOWN", true);
    gw.tick(DT);
    let c = gw.camera().borrow();
    assert!((c.yaw - (yaw0 - 90.0 * DT)).abs() < 1e-4);
    assert!((c.pitch - (pitch0 - 90.0 * DT)).abs() < 1e-4);
}
