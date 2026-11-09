//! A sub-object's collider built from another sub-object, its collision mesh (#1960):
//! the low-poly sibling builds it, is never drawn, survives a save and load, and a
//! name the model lacks falls back to the own mesh with one warning.

use crate::asset::{sidecar, MeshColliderKind, SubObjectCollider};
use crate::physics::collider_world_triangles;
use crate::scene::model_instance::instantiate_model;
use crate::scene::serialize::{apply_scene_data, to_scene_data};
use crate::scene::Scene;

/// A bumpy `Chair` of `n`×`n` quads (2n² triangles, 0.1 high) and a two-triangle `Chair_col`,
/// written as `file`, its sidecar's `Chair` collider built from `mesh`. Returns the path.
pub(crate) fn chair(file: &str, n: u32, mesh: Option<&str>) -> String {
    let mut obj = String::from("o Chair\n");
    for z in 0..=n {
        for x in 0..=n {
            let y = if (x + z) % 2 == 1 { 0.1 } else { 0.0 };
            obj += &format!("v {} {y} {}\n", x as f32 / n as f32, z as f32 / n as f32);
        }
    }
    let at = |x: u32, z: u32| z * (n + 1) + x + 1;
    for z in 0..n {
        for x in 0..n {
            let (a, b, c, d) = (at(x, z), at(x + 1, z), at(x + 1, z + 1), at(x, z + 1));
            obj += &format!("f {a} {b} {c}\nf {a} {c} {d}\n");
        }
    }
    let base = (n + 1) * (n + 1);
    obj += "o Chair_col\nv 0 0 0\nv 1 0 0\nv 1 0.5 1\nv 0 0.5 1\n";
    obj += &format!("f {} {} {}\n", base + 1, base + 2, base + 3);
    obj += &format!("f {} {} {}\n", base + 1, base + 3, base + 4);
    let path = crate::test_temp::dir().join(file);
    std::fs::write(&path, obj).unwrap();
    let mut settings = sidecar::ImportSettings::default();
    let entry = SubObjectCollider {
        kind: MeshColliderKind::TriMesh,
        mesh: mesh.map(str::to_string),
    };
    settings.colliders.insert("Chair".into(), entry);
    sidecar::save(&path, &settings).unwrap();
    path.to_string_lossy().into_owned()
}

/// The model at `path` spawned whole, and its child entities' names and ids.
pub(crate) fn spawn(path: &str) -> (Scene, Vec<(String, u32)>) {
    let asset = crate::asset::import_and_sync_sidecar(std::path::Path::new(path)).unwrap();
    let mut scene = Scene::new();
    let root = instantiate_model(&mut scene, path, &asset, glam::Vec3::ZERO);
    let kids = scene.world.children(root).into_iter();
    let named = kids
        .map(|id| (scene.world.name(id).unwrap().clone(), id))
        .collect();
    (scene, named)
}

pub(crate) fn triangles(scene: &Scene, id: u32) -> usize {
    collider_world_triangles(scene, id).map_or(0, |t| t.triangles.len())
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
    use crate::scene::authoring::{add_component, remove_component, ComponentKind};
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
    assert!(crate::scene::rehydrate::warned_of(&format!("{path}::Nope")));
}
