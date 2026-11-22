use super::*;
use glam::Vec3;
use rusty_scene::Scene;
use std::cell::RefCell;
use std::rc::Rc;

pub(crate) const DT: f32 = 1.0 / 60.0;

pub(crate) fn world_with(scene: Rc<RefCell<Scene>>) -> GameWorld {
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

#[test]
fn accessors_expose_shared_handles() {
    let gw = empty_world();
    assert!(Rc::ptr_eq(gw.scene(), &gw.world.scene));
    assert!(Rc::ptr_eq(gw.input(), &gw.resources.input));
    assert!(Rc::ptr_eq(gw.nav(), &gw.resources.nav));
    assert!(Rc::ptr_eq(gw.console(), &gw.resources.console));
    assert!(Rc::ptr_eq(gw.camera(), &gw.resources.camera));
    assert!(Rc::ptr_eq(gw.time(), &gw.resources.time));
    assert!(!gw.play_requested());
    assert_eq!(gw.play_frame(), 0);
    let _ = gw.script_manager();
}

#[test]
fn set_playing_flips_state() {
    let mut gw = empty_world();
    assert!(!gw.play_requested());
    gw.set_playing(true);
    assert!(gw.play_requested());
    gw.set_playing(false);
    assert!(!gw.play_requested());
}

#[test]
fn transition_is_none_while_steady() {
    let mut gw = empty_world();
    assert!(gw.tick(DT) == PlayTransition::None);
    gw.set_playing(true);
    assert!(gw.tick(DT) == PlayTransition::Entered);
    assert!(gw.tick(DT) == PlayTransition::None);
}

#[test]
fn entering_play_resets_frame_and_time() {
    let mut gw = empty_world();
    gw.time().borrow_mut().advance(0.5);
    gw.resources.play_frame = 99;
    gw.set_playing(true);
    let t = gw.tick(DT);
    assert!(t == PlayTransition::Entered);
    assert_eq!(gw.time().borrow().frame_count, 1);
    assert_eq!(gw.play_frame(), 1);
}

#[test]
fn exiting_play_clears_runtime_state() {
    let mut gw = empty_world();
    gw.set_playing(true);
    gw.tick(DT);
    assert!(gw.resources.physics.borrow().is_some());
    gw.set_playing(false);
    let t = gw.tick(DT);
    assert!(t == PlayTransition::Exited);
    assert!(gw.resources.physics.borrow().is_none());
    assert_eq!(gw.play_frame(), 0);
}

#[test]
fn stop_restores_the_edit_scene_snapshot() {
    let mut s = Scene::new();
    let id = s.add_entity("Mover".to_string());
    s.world.transform_mut(id).unwrap().position = Vec3::new(1.0, 0.0, 0.0);
    let scene = Rc::new(RefCell::new(s));
    let mut gw = world_with(Rc::clone(&scene));

    gw.set_playing(true);
    gw.tick(DT);
    scene.borrow_mut().world.transform_mut(id).unwrap().position = Vec3::new(9.0, 9.0, 9.0);
    gw.set_playing(false);
    gw.tick(DT);
    let restored = scene.borrow().world.transform(id).unwrap().position;
    assert!(
        restored.abs_diff_eq(Vec3::new(1.0, 0.0, 0.0), 1e-6),
        "play-mode mutation leaked into the edit scene: {restored:?}"
    );
}

#[test]
fn tick_scales_dt_by_time_scale() {
    let mut gw = empty_world();
    gw.set_playing(true);
    gw.tick(DT);
    gw.time().borrow_mut().set_time_scale(0.5);
    gw.tick(DT);
    assert!((gw.resources.frame_dt - DT * 0.5).abs() < 1e-7);
    assert!((gw.time().borrow().unscaled_delta_time - DT).abs() < 1e-7);
}

#[test]
fn play_frame_accumulates_each_tick() {
    let mut gw = empty_world();
    gw.set_playing(true);
    for _ in 0..5 {
        gw.tick(DT);
    }
    assert_eq!(gw.play_frame(), 5);
}
