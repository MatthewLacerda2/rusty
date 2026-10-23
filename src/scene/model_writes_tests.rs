//! A model the engine re-writes reloads into the entities that use it (#1242): the
//! CPU mesh and its mesh collider's box follow the file, and a write of another file
//! leaves them alone.

use std::cell::RefCell;
use std::rc::Rc;

use glam::Vec3;

use super::ModelWrites;
use crate::components::{ColliderComponent, ColliderShape};
use crate::core::fs::written::contract::{follows_rebakes, Bake};
use crate::core::fs::written::write;
use crate::scene::{asset_mesh_component, Scene};

/// A two-triangle unit quad (6 indices).
const QUAD: &str = "o Model\nv 0 0 0\nv 1 0 0\nv 1 1 0\nv 0 1 0\nf 1 2 3\nf 1 3 4\n";
/// One triangle twice the quad's size (3 indices), under the same object name.
const TRIANGLE: &str = "o Model\nv 0 0 0\nv 2 0 0\nv 2 2 0\nf 1 2 3\n";

fn model(dir: &str) -> String {
    let dir = crate::test_temp::dir().join(dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("model.obj").to_string_lossy().into_owned()
}

fn bake(path: &str, which: Bake) {
    let text = match which {
        Bake::A => QUAD,
        Bake::B => TRIANGLE,
    };
    write(std::path::Path::new(path), text.as_bytes()).unwrap();
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

fn collider_box(scene: &Scene, id: u32) -> (Vec3, Vec3) {
    match scene.world.collider(id).unwrap().shape {
        ColliderShape::Mesh {
            local_min,
            local_max,
            ..
        } => (local_min, local_max),
        ref other => panic!("not a mesh collider: {other:?}"),
    }
}

#[test]
fn a_rewritten_model_reloads_into_its_entities_mesh_and_collider() {
    let path = model("rusty_model_writes_rewrite");
    bake(&path, Bake::A);
    let mut scene = Scene::new();
    let id = spawn(&mut scene, &path);
    let mut writes = ModelWrites::default();
    assert_eq!(index_count(&scene, id), 6);
    scene.world.mesh(id).unwrap().is_dirty.set(false);

    bake(&path, Bake::B);
    assert_eq!(writes.follow(&mut scene), 1);
    assert_eq!(index_count(&scene, id), 3, "the CPU mesh is the new file's");
    assert!(scene.world.mesh(id).unwrap().is_dirty.get(), "re-uploaded");
    assert_eq!(
        collider_box(&scene, id),
        (Vec3::ZERO, Vec3::new(2.0, 2.0, 0.0)),
        "the mesh collider is refit"
    );
    let (_, max) = scene.world.collider_bounds(id).unwrap();
    assert_eq!(max, Vec3::new(2.0, 2.0, 0.0), "and its world box with it");
    assert_eq!(writes.follow(&mut scene), 0, "a note is followed once");
}

#[test]
fn a_write_of_another_file_leaves_the_mesh_alone() {
    let path = model("rusty_model_writes_unrelated");
    bake(&path, Bake::A);
    let mut scene = Scene::new();
    let id = spawn(&mut scene, &path);
    let mut writes = ModelWrites::default();
    // Changed on disk without a note: only a note sends a mesh back to its file.
    std::fs::write(&path, TRIANGLE).unwrap();
    let other = model("rusty_model_writes_other");
    bake(&other, Bake::B);

    assert_eq!(writes.follow(&mut scene), 0);
    assert_eq!(index_count(&scene, id), 6, "not re-read");
    assert_eq!(collider_box(&scene, id), (Vec3::ZERO, Vec3::ONE));
}

/// An edit-mode game over `scene`.
fn game_over(scene: Rc<RefCell<Scene>>) -> crate::app::GameWorld {
    let nav = crate::navigation::NavigationGraph::new(-1.0, 1.0, -1.0, 1.0, 1.0);
    crate::app::GameWorld::new(
        scene,
        Rc::new(RefCell::new(crate::core::input::InputState::new())),
        Rc::new(RefCell::new(nav)),
        Rc::new(RefCell::new(crate::scripting::ConsoleLogs::new())),
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
