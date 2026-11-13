//! A sub-object's collider built from another sub-object, its collision mesh (#1960):
//! the low-poly sibling builds it, is never drawn, survives a save and load, and a
//! name the model lacks falls back to the own mesh with one warning.

use crate::model_instance::instantiate_model;
use crate::serialize::{apply_scene_data, to_scene_data};
use crate::Scene;

/// The [`chair::write`](super::chair::write) model as `file` in the test's temp dir;
/// its path, slashed (`core::paths::to_slash`) so a test can put it in a Lua literal.
pub(super) fn chair(file: &str, n: u32, mesh: Option<&str>) -> String {
    let path = rusty_test_support::temp::dir().join(file);
    super::chair::write(&path, n, mesh).unwrap();
    rusty_core::paths::to_slash(&path)
}

/// The model at `path` spawned whole, and its child entities' names and ids.
pub(super) fn spawn(path: &str) -> (Scene, Vec<(String, u32)>) {
    let asset = rusty_asset::import_and_sync_sidecar(std::path::Path::new(path)).unwrap();
    let mut scene = Scene::new();
    let root = instantiate_model(&mut scene, path, &asset, glam::Vec3::ZERO);
    let kids = scene.world.children(root).into_iter();
    let named = kids
        .map(|id| (scene.world.name(id).unwrap().clone(), id))
        .collect();
    (scene, named)
}

/// The triangles of the mesh `id`'s active collider is built from: its collision
/// mesh, else its entity's Mesh, else its kept `shared_mesh`. Scene's half of the
/// collider physics builds (`physics::shape::build::collider_inputs` makes the same
/// choice; the shape it builds from the mesh is physics' tests').
pub(super) fn triangles(scene: &Scene, id: u32) -> usize {
    let Some(c) = scene.world.collider(id).filter(|c| c.active) else {
        return 0;
    };
    let count = |indices: &[u32]| indices.len() / 3;
    c.built_collision_mesh()
        .map(|m| count(&m.indices))
        .or_else(|| scene.world.mesh(id).map(|m| count(&m.indices)))
        .or_else(|| c.shared_mesh.as_ref().map(|m| count(&m.indices)))
        .unwrap_or(0)
}

#[test]
fn the_low_poly_sibling_builds_the_collider_and_is_not_drawn() {
    let path = chair("rusty_1960_chair.obj", 12, Some("Chair_col"));
    let (scene, kids) = spawn(&path);
    let names: Vec<&str> = kids.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(names, ["Chair"], "no entity draws the collision mesh");
    let id = kids[0].1;
    assert_eq!(scene.world.mesh(id).unwrap().indices.len(), 12 * 12 * 6);
    assert_eq!(
        triangles(&scene, id),
        2,
        "physics collides with the sibling"
    );
    let c = scene.world.collider(id).unwrap();
    let shared = c.collision_mesh.as_ref().expect("the collision mesh");
    assert!(shared
        .asset_ref
        .as_deref()
        .unwrap()
        .ends_with("::Chair_col"));
    assert_eq!(c.aabb_max.y, 0.5, "the box is the sibling's");
}

#[test]
fn the_collision_mesh_is_rebuilt_on_load() {
    let path = chair("rusty_1960_chair_load.obj", 6, Some("Chair_col"));
    let (scene, kids) = spawn(&path);
    let text = serde_json::to_string(&to_scene_data(&scene)).unwrap();
    assert!(!text.contains("positions"), "only the reference is saved");
    let mut loaded = Scene::new();
    apply_scene_data(&mut loaded, serde_json::from_str(&text).unwrap());
    let id = kids[0].1;
    assert_eq!(triangles(&loaded, id), 2);
    assert_eq!(loaded.world.collider(id).unwrap().aabb_max.y, 0.5);
}

#[test]
fn an_undone_edit_keeps_the_collision_mesh() {
    let path = chair("rusty_1960_chair_undo.obj", 4, Some("Chair_col"));
    let (mut scene, kids) = spawn(&path);
    let id = kids[0].1;
    scene.clear_undo();
    scene.begin_edits();
    scene.world.transform_mut(id).unwrap().position = glam::Vec3::X;
    scene.commit_edits(None);
    assert!(scene.undo().is_some());
    assert_eq!(triangles(&scene, id), 2);
}

#[test]
fn removing_and_adding_the_mesh_keeps_the_collision_mesh() {
    use crate::authoring::{add_component, remove_component, ComponentKind};
    let path = chair("rusty_1960_chair_remove.obj", 4, Some("Chair_col"));
    let (mut scene, kids) = spawn(&path);
    let id = kids[0].1;
    assert!(remove_component(&mut scene, id, ComponentKind::Mesh));
    assert_eq!(triangles(&scene, id), 2, "still the sibling, not the Chair");
    assert!(add_component(&mut scene, id, ComponentKind::Mesh));
    let c = scene.world.collider(id).unwrap();
    assert!(c.collision_mesh.is_some(), "Unity keeps sharedMesh");
    assert_eq!(c.shared_mesh, None, "the kept copy goes");
    assert_eq!(triangles(&scene, id), 2);
    assert_eq!(scene.world.collider(id).unwrap().aabb_max.y, 0.5);
}

#[test]
fn a_missing_name_falls_back_to_the_own_mesh_with_one_warning() {
    let path = chair("rusty_1960_chair_missing.obj", 3, Some("Nope"));
    let (scene, kids) = spawn(&path);
    let (_, again) = spawn(&path);
    let id = kids.iter().find(|(n, _)| n == "Chair").unwrap().1;
    let kept = scene.world.collider(id).unwrap().collision_mesh.clone();
    let kept = kept.expect("the name is kept, to pick up once the model has it");
    assert!(kept.indices.is_empty());
    assert_eq!(triangles(&scene, id), 3 * 3 * 2, "the own mesh");
    assert_eq!(
        again.len(),
        2,
        "named by no one, Chair_col is a plain object"
    );
    let key = super::warn_key(&format!("{path}::Nope"));
    assert!(crate::rehydrate::warned_of(&key));
}
