//! The edit frame runs on a frozen frame outside Play (#1242): while a pause
//! outlives Stop, a model the engine re-wrote still reloads into its entities.

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use super::*;
use crate::core::fs::written::write;
use crate::core::input::InputState;
use crate::navigation::NavigationGraph;
use crate::scene::{asset_mesh_component, Scene};
use crate::scripting::ConsoleLogs;

#[test]
fn a_frozen_frame_outside_play_reloads_a_rewritten_model() {
    let dir = crate::test_temp::dir().join("rusty_frozen_edit_frame");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("model.obj");
    let quad = "o Model\nv 0 0 0\nv 1 0 0\nv 1 1 0\nv 0 1 0\nf 1 2 3\nf 1 3 4\n";
    write(&path, quad.as_bytes()).unwrap();
    let scene = Rc::new(RefCell::new(Scene::new()));
    let id = scene.borrow_mut().add_entity("Model".to_string());
    let reference = format!("{}::Model", path.to_string_lossy());
    let mesh = asset_mesh_component(&reference);
    scene.borrow_mut().world.set_mesh(id, Some(mesh));
    let mut game = GameWorld::new(
        Rc::clone(&scene),
        Rc::new(RefCell::new(InputState::new())),
        Rc::new(RefCell::new(NavigationGraph::new(
            -1.0, 1.0, -1.0, 1.0, 1.0,
        ))),
        Rc::new(RefCell::new(ConsoleLogs::new())),
    );
    game.time().borrow_mut().pause();
    let triangle = "o Model\nv 0 0 0\nv 2 0 0\nv 2 2 0\nf 1 2 3\n";
    write(Path::new(&path), triangle.as_bytes()).unwrap();

    advance_sim(&mut game, 0.02, &mut Default::default());
    assert!(!game.is_playing());
    let indices = scene.borrow().world.mesh(id).unwrap().indices.len();
    assert_eq!(
        indices, 3,
        "one frozen frame later the mesh is the new file's"
    );
}
