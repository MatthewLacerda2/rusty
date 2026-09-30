//! Unit tests for the play-loop systems in `play.rs` (#450): entering Play never
//! moves the camera on the engine's own initiative (no entity is looked up by
//! name), and the navmesh rebake keeps its 60-frame cadence.

use std::cell::RefCell;
use std::rc::Rc;

use glam::Vec3;

use crate::app::GameWorld;
use crate::core::input::InputState;
use crate::navigation::NavigationGraph;
use crate::scene::Scene;
use crate::scripting::ConsoleLogs;

const DT: f32 = 1.0 / 60.0;

/// A world whose scene holds an entity named `Player` — the name the engine used
/// to key its camera snap on — with the camera parked somewhere else entirely.
fn world_with_a_player() -> GameWorld {
    let mut scene = Scene::new();
    let id = scene.add_entity("Player".to_string());
    scene.world.transform_mut(id).unwrap().position = Vec3::new(2.0, 0.0, 3.0);
    let gw = GameWorld::new(
        Rc::new(RefCell::new(scene)),
        Rc::new(RefCell::new(InputState::new())),
        Rc::new(RefCell::new(NavigationGraph::new(
            -20.0, 20.0, -20.0, 20.0, 1.0,
        ))),
        Rc::new(RefCell::new(ConsoleLogs::new())),
    );
    {
        let mut cam = gw.camera().borrow_mut();
        cam.position = Vec3::new(-7.0, 4.0, 9.0);
        cam.yaw = 12.0;
        cam.pitch = 5.0;
    }
    gw
}

fn assert_camera_untouched(gw: &GameWorld) {
    let cam = gw.camera().borrow();
    assert_eq!(cam.position, Vec3::new(-7.0, 4.0, 9.0));
    assert_eq!(cam.yaw, 12.0);
    assert_eq!(cam.pitch, 5.0);
}

#[test]
fn entering_play_leaves_the_camera_where_the_scene_put_it() {
    let mut gw = world_with_a_player();
    gw.set_playing(true);
    gw.tick(DT);
    assert_camera_untouched(&gw);
}

#[test]
fn standalone_boot_leaves_the_camera_where_the_scene_put_it() {
    let mut gw = world_with_a_player();
    gw.boot_standalone();
    gw.tick(DT);
    assert_camera_untouched(&gw);
}

#[test]
fn the_navmesh_rebakes_every_sixty_play_frames() {
    let mut gw = world_with_a_player();
    gw.set_playing(true);
    let generation = |gw: &GameWorld| gw.nav().borrow().bake_generation;
    assert_eq!(generation(&gw), 0);
    gw.tick(DT); // play frame 0 bakes on entry
    assert_eq!(generation(&gw), 1);
    for _ in 0..59 {
        gw.tick(DT); // play frames 1..=59: no rebake
    }
    assert_eq!(generation(&gw), 1);
    gw.tick(DT); // play frame 60
    assert_eq!(generation(&gw), 2);
    for _ in 0..60 {
        gw.tick(DT);
    }
    assert_eq!(generation(&gw), 3, "play frame 120");
}
