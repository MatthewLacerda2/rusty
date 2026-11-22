//! A scripted chase camera moves the main camera entity in `LateUpdate`; the harness, Game view and
//! audio listener all look from it, and moving its Transform moves `Camera.*`.

use std::cell::RefCell;
use std::rc::Rc;

use glam::Vec3;
use rusty::app::GameWorld;
use rusty::edit::authoring::default_camera;
use rusty::render::build_camera_stack;
use rusty::scene::{game_camera_from_scene, Camera, Scene};

use crate::play::{attach_script, playing, run};

const CHASE: &str = "local x = 0\n\
    return { LateUpdate = function(id)\n\
    x = x + 1\n\
    Camera.SetPosition(x, 3, 8)\n\
    Camera.SetYaw(-90)\n\
    Camera.SetPitch(-20)\nend }";

fn session(script: &str, file: &str) -> (GameWorld, Rc<RefCell<Scene>>, u32) {
    let mut scene = Scene::new();
    let cam = scene.add_entity("MainCamera".to_string());
    scene.world.set_camera(cam, Some(default_camera()));
    let driver = scene.add_entity("Driver".to_string());
    attach_script(&mut scene, driver, file, script);
    let scene = Rc::new(RefCell::new(scene));
    (playing(Rc::clone(&scene), 10.0), scene, cam)
}

fn assert_pose(what: &str, cam: &Camera, position: Vec3, yaw: f32, pitch: f32) {
    let close = cam.position.abs_diff_eq(position, 1e-3)
        && (cam.yaw - yaw).abs() < 1e-2
        && (cam.pitch - pitch).abs() < 1e-2;
    let got = (cam.position, cam.yaw, cam.pitch);
    assert!(
        close,
        "{what}: {got:?}, expected {:?}",
        (position, yaw, pitch)
    );
}

#[test]
fn a_scripted_camera_move_is_one_pose_on_every_path() {
    let (mut game, scene, cam) = session(CHASE, "rusty_1619_chase.lua");
    run(&mut game, 3);
    let want = (Vec3::new(3.0, 3.0, 8.0), -90.0, -20.0);
    let main = game.main_view();
    let scene = scene.borrow();
    let stack = build_camera_stack(&main, &scene, true, &Default::default());
    assert_eq!(stack.entities[0], Some(cam));
    assert_pose("screen stack", &stack.cameras[0], want.0, want.1, want.2);
    // The editor's Game view, and the camera entity itself.
    let resource = game.camera().borrow();
    let game_view = game_camera_from_scene(&resource, &scene);
    assert_pose("game view", &game_view, want.0, want.1, want.2);
    let entity = scene.world.local_to_world(cam).col(3).truncate();
    assert!(
        entity.abs_diff_eq(want.0, 1e-3),
        "the entity moved: {entity:?}"
    );
    // The view camera resource the audio listener reads.
    assert_pose("view camera", &resource, want.0, want.1, want.2);
}

#[test]
fn moving_the_camera_entity_moves_what_camera_reads() {
    let (mut game, _scene, cam) = session("return {}", "rusty_1619_idle.lua");
    run(&mut game, 1);
    let eval = |line: &str| game.script_manager().eval(line).expect("evaluates");
    eval(&format!("Transform.SetPosition({cam}, 1, 2, 3)"));
    let read = eval("string.format('%.1f %.1f %.1f', Camera.GetPosition())");
    assert_eq!(read, "1.0 2.0 3.0");
    // The resource mirrors it from the next tick on.
    run(&mut game, 1);
    let at = game.camera().borrow().position;
    assert!(at.abs_diff_eq(Vec3::new(1.0, 2.0, 3.0), 1e-4), "{at:?}");
}
