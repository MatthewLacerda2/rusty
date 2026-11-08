//! The edit frame runs on a frozen frame outside Play (#1242): while a pause
//! outlives Stop, a model the engine re-wrote still reloads into its entities, and
//! the UI reads a texture size (#1241).

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use super::*;
use crate::components::ImageComponent;
use crate::core::fs::written::write;
use crate::core::input::InputState;
use crate::navigation::NavigationGraph;
use crate::scene::{asset_mesh_component, Scene};
use crate::scripting::ConsoleLogs;

fn edit_game(scene: &Rc<RefCell<Scene>>) -> GameWorld {
    GameWorld::new(
        Rc::clone(scene),
        Rc::new(RefCell::new(InputState::new())),
        Rc::new(RefCell::new(NavigationGraph::new(
            -1.0, 1.0, -1.0, 1.0, 1.0,
        ))),
        Rc::new(RefCell::new(ConsoleLogs::new())),
    )
}

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
    let mut game = edit_game(&scene);
    game.time().borrow_mut().pause();
    let triangle = "o Model\nv 0 0 0\nv 2 0 0\nv 2 2 0\nf 1 2 3\n";
    write(Path::new(&path), triangle.as_bytes()).unwrap();

    advance_sim(&mut game, 0.02);
    assert!(!game.scene().borrow().is_playing());
    let indices = scene.borrow().world.mesh(id).unwrap().indices.len();
    assert_eq!(
        indices, 3,
        "one frozen frame later the mesh is the new file's"
    );
}

#[test]
fn a_frozen_frame_outside_play_reads_ui_texture_sizes() {
    let dir = crate::test_temp::dir().join("rusty_frozen_ui_sizes");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("icon.png").to_string_lossy().into_owned();
    let scene = Rc::new(RefCell::new(Scene::new()));
    let id = scene.borrow_mut().add_entity("Icon".to_string());
    let img = ImageComponent {
        texture: Some(path.clone().into()),
        ..Default::default()
    };
    scene.borrow_mut().world.set_image(id, Some(img));
    let mut game = edit_game(&scene);
    game.time().borrow_mut().pause();
    let image = crate::procgen::Image::new(8);
    crate::procgen::bake::bake_to_png(&image, crate::procgen::Slot::BaseColor, &path).unwrap();

    advance_sim(&mut game, 0.02);
    let size = game.resources.screen.borrow().textures().get(&path);
    assert_eq!(size, Some(glam::Vec2::splat(8.0)), "read on a frozen frame");
}
