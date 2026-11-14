//! An edit-mode tick reloads a model the engine re-writes (#1242) into the
//! entities that use it. The reload itself is `scene::rehydrate::model_writes_tests`.

use std::cell::RefCell;
use std::rc::Rc;

use glam::Vec3;

use rusty_components::ColliderComponent;
use rusty_scene::{asset_mesh_component, Scene};
use rusty_test_support::rebakes::{follows_rebakes, Bake};

fn model(dir: &str) -> String {
    let dir = rusty_test_support::temp::dir().join(dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("model.obj").to_string_lossy().into_owned()
}

/// A two-triangle quad (6 indices) or one triangle (3), under the object `Model`.
fn bake(path: &str, which: Bake) {
    let text = match which {
        Bake::A => "o Model\nv 0 0 0\nv 1 0 0\nv 1 1 0\nv 0 1 0\nf 1 2 3\nf 1 3 4\n",
        Bake::B => "o Model\nv 0 0 0\nv 2 0 0\nv 2 2 0\nf 1 2 3\n",
    };
    rusty_core::fs::written::write(std::path::Path::new(path), text.as_bytes()).unwrap();
}

/// An entity carrying the model's mesh and a mesh collider sized to it.
fn spawn(scene: &mut Scene, path: &str) -> u32 {
    let id = scene.add_entity("Model".to_string());
    let mesh = asset_mesh_component(&format!("{path}::Model"));
    scene.world.set_mesh(id, Some(mesh));
    let collider = ColliderComponent::from_mesh_bounds(false, Vec3::ZERO, Vec3::ONE);
    scene.world.set_collider(id, Some(collider));
    id
}

fn index_count(scene: &Scene, id: u32) -> usize {
    scene.world.mesh(id).unwrap().indices.len()
}

/// An edit-mode game over `scene`.
pub(super) fn game_over(scene: Rc<RefCell<Scene>>) -> crate::GameWorld {
    let nav = rusty_navigation::NavigationGraph::new(-1.0, 1.0, -1.0, 1.0, 1.0);
    crate::GameWorld::new(
        scene,
        Rc::new(RefCell::new(rusty_core::input::InputState::new())),
        Rc::new(RefCell::new(nav)),
        Rc::new(RefCell::new(rusty_script::scripting::ConsoleLogs::new())),
    )
}

#[test]
fn an_edit_mode_tick_follows_rebakes() {
    let path = model("rusty_model_writes_tick");
    let scene = Rc::new(RefCell::new(Scene::new()));
    let id = spawn(&mut scene.borrow_mut(), &path);
    let mut game = game_over(Rc::clone(&scene));
    follows_rebakes(
        &path,
        bake,
        |_| {
            game.tick(1.0 / 60.0);
            index_count(&scene.borrow(), id)
        },
        |count, which| *count == if which == Bake::A { 6 } else { 3 },
    );
}
