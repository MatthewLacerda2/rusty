//! Unit tests for the play/stop boundary in `GameWorld` (game.rs): pitch clamping,
//! the transition state machine, the scaled-dt threading, and the
//! snapshot-capture-on-Play / restore-on-Stop invariant. Determinism contract:
//! fixed dt, no wall-clock, no RNG.

use super::*;
use crate::scene::Scene;
use glam::Vec3;
use std::cell::RefCell;
use std::rc::Rc;

fn world_with(scene: Rc<RefCell<Scene>>) -> GameWorld {
    let input = Rc::new(RefCell::new(InputState::new()));
    let nav = Rc::new(RefCell::new(NavigationGraph::new(
        -20.0, 20.0, -20.0, 20.0, 1.0,
    )));
    let console = Rc::new(RefCell::new(ConsoleLogs::new()));
    GameWorld::new(scene, input, nav, console)
}

fn empty_world() -> GameWorld {
    world_with(Rc::new(RefCell::new(Scene::new())))
}

const DT: f32 = 1.0 / 60.0;

#[test]
fn editor_fly_pitch_clamps_at_eighty() {
    let mut gw = empty_world();
    gw.camera().borrow_mut().pitch = 79.5;
    gw.input().borrow_mut().set_key_state("UP", true);
    for _ in 0..120 {
        gw.tick(DT);
    }
    assert!((gw.camera().borrow().pitch - 80.0).abs() < 1e-4);

    let mut gw = empty_world();
    gw.camera().borrow_mut().pitch = -79.5;
    gw.input().borrow_mut().set_key_state("DOWN", true);
    for _ in 0..120 {
        gw.tick(DT);
    }
    assert!((gw.camera().borrow().pitch + 80.0).abs() < 1e-4);
}

#[test]
fn transition_is_none_while_steady() {
    let mut gw = empty_world();
    assert!(gw.tick(DT) == PlayTransition::None); // stays in edit mode
    gw.set_playing(true);
    assert!(gw.tick(DT) == PlayTransition::Entered);
    assert!(gw.tick(DT) == PlayTransition::None); // already playing
}

#[test]
fn entering_play_resets_frame_and_time() {
    let mut gw = empty_world();
    // Dirty the clock so we can see the reset on enter_play.
    gw.time().borrow_mut().advance(0.5);
    gw.resources.play_frame = 99;
    gw.set_playing(true);
    let t = gw.tick(DT);
    assert!(t == PlayTransition::Entered);
    // enter_play resets frame_count to 0 then the tick's own advance makes it 1.
    assert_eq!(gw.time().borrow().frame_count, 1);
    // advance_frame ran once this tick.
    assert_eq!(gw.play_frame(), 1);
}

#[test]
fn exiting_play_clears_runtime_state() {
    let mut gw = empty_world();
    gw.set_playing(true);
    gw.tick(DT); // Entered: builds physics, sets play_frame
    assert!(gw.resources.physics.borrow().is_some());
    gw.resources.pathfinding_points = vec![Vec3::ZERO];
    gw.set_playing(false);
    let t = gw.tick(DT);
    assert!(t == PlayTransition::Exited);
    // exit_play tore physics down, cleared the debug path, reset the counter.
    assert!(gw.resources.physics.borrow().is_none());
    assert!(gw.pathfinding_points().is_empty());
    assert_eq!(gw.play_frame(), 0);
}

#[test]
fn stop_restores_the_edit_scene_snapshot() {
    let mut s = Scene::new();
    let id = s.add_entity("Mover".to_string());
    s.get_entity_mut(id).unwrap().transform.position = Vec3::new(1.0, 0.0, 0.0);
    let scene = Rc::new(RefCell::new(s));
    let mut gw = world_with(Rc::clone(&scene));

    gw.set_playing(true);
    gw.tick(DT); // Entered: snapshot captured
                 // Mutate the entity mid-play.
    scene
        .borrow_mut()
        .get_entity_mut(id)
        .unwrap()
        .transform
        .position = Vec3::new(9.0, 9.0, 9.0);
    gw.set_playing(false);
    gw.tick(DT); // Exited: restore snapshot
    let restored = scene.borrow().get_entity(id).unwrap().transform.position;
    assert!(
        restored.abs_diff_eq(Vec3::new(1.0, 0.0, 0.0), 1e-6),
        "play-mode mutation leaked into the edit scene: {restored:?}"
    );
}

#[test]
fn tick_scales_dt_by_time_scale() {
    let mut gw = empty_world();
    gw.set_playing(true);
    gw.tick(DT); // Entered resets clock; advance applies scale 1.0
    gw.time().borrow_mut().set_time_scale(0.5);
    gw.tick(DT);
    // The schedule sees the scaled delta the tick stored in frame_dt.
    assert!((gw.resources.dt() - DT * 0.5).abs() < 1e-7);
    // Raw (unscaled) delta is preserved on the clock.
    assert!((gw.time().borrow().unscaled_delta_time - DT).abs() < 1e-7);
}

#[test]
fn play_frame_accumulates_each_tick() {
    let mut gw = empty_world();
    gw.set_playing(true);
    for _ in 0..5 {
        gw.tick(DT);
    }
    // enter_play set 0, then five ticks each run advance_frame once.
    assert_eq!(gw.play_frame(), 5);
}

#[test]
fn snap_camera_to_player_positions_behind_player() {
    let mut s = Scene::new();
    let id = s.add_entity("Player".to_string());
    s.get_entity_mut(id).unwrap().transform.position = Vec3::new(2.0, 0.0, 3.0);
    let mut gw = world_with(Rc::new(RefCell::new(s)));
    gw.set_playing(true);
    gw.tick(DT); // enter_play calls snap_camera_to_player
    let cam = gw.camera().borrow();
    assert!(cam.position.abs_diff_eq(Vec3::new(2.0, 1.5, -1.5), 1e-5));
    assert!((cam.yaw - 90.0).abs() < 1e-5);
    assert!((cam.pitch + 10.0).abs() < 1e-5);
}
