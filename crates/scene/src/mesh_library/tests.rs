//! The scene mesh library (#2059) across save and load, Play and Stop, Undo, a
//! prefab and a Mesh collider.

use glam::Vec3;

use crate::{apply_scene_data, to_scene_data, Scene, SceneSnapshot};
use rusty_components::mesh::MeshData;
use rusty_components::{ColliderComponent, ColliderShape};

/// A unit tetrahedron, normals computed.
fn tetra() -> MeshData {
    MeshData {
        positions: vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 0.0, -1.0],
            [0.0, 1.0, 0.0],
        ],
        indices: vec![0, 2, 1, 0, 1, 3, 1, 2, 3, 2, 0, 3],
        ..Default::default()
    }
}

fn triangle(size: f32) -> MeshData {
    MeshData {
        positions: vec![[0.0; 3], [size, 0.0, 0.0], [0.0, size, 0.0]],
        indices: vec![0, 1, 2],
        ..Default::default()
    }
}

fn scene_drawing(data: MeshData) -> (Scene, u32) {
    let mut scene = Scene::new();
    let id = scene.add_entity("Piece".into());
    scene.define_mesh("piece", data).unwrap();
    scene.set_shared_mesh(id, "piece").unwrap();
    (scene, id)
}

fn vertex_count(scene: &Scene, id: u32) -> usize {
    scene.world.mesh(id).map_or(0, |m| m.vertices.len())
}

#[test]
fn a_saved_scene_mesh_loads_back_with_its_geometry() {
    let (scene, id) = scene_drawing(tetra());
    let json = serde_json::to_string(&to_scene_data(&scene)).unwrap();
    assert!(json.contains(r#""meshes":{"piece":{"positions""#), "{json}");
    let mut loaded = Scene::new();
    apply_scene_data(&mut loaded, serde_json::from_str(&json).unwrap());
    assert_eq!(loaded.meshes["piece"], scene.meshes["piece"]);
    let mesh = loaded.world.mesh(id).unwrap();
    assert_eq!((mesh.vertices.len(), mesh.indices.len()), (4, 12));
    assert_eq!(mesh.revision, loaded.meshes["piece"].revision);
    let plain = serde_json::to_string(&to_scene_data(&Scene::new())).unwrap();
    assert!(!plain.contains("meshes"), "an empty library is left out");
}

#[test]
fn stop_drops_a_play_mesh_and_puts_the_edit_one_back() {
    let (mut scene, id) = scene_drawing(tetra());
    let snapshot = SceneSnapshot::capture(&scene);
    scene.define_mesh("piece", triangle(1.0)).unwrap();
    scene.define_mesh("debris", triangle(2.0)).unwrap();
    assert_eq!(vertex_count(&scene, id), 3);
    let played = scene.world.mesh(id).unwrap().revision;
    snapshot.restore(&mut scene);
    assert!(!scene.meshes.contains_key("debris"));
    assert_eq!(vertex_count(&scene, id), 4);
    let mesh = scene.world.mesh(id).unwrap();
    assert!(
        mesh.revision > played,
        "newer than Play's, so the renderer re-uploads"
    );
}

#[test]
fn undo_of_a_replace_brings_the_old_geometry_back_with_a_newer_revision() {
    let (mut scene, id) = scene_drawing(tetra());
    scene.begin_edits();
    scene.define_mesh("piece", triangle(1.0)).unwrap();
    scene.commit_edits(Some("Replace"));
    let replaced = scene.world.mesh(id).unwrap().revision;
    assert_eq!(scene.undo().as_deref(), Some("Replace"));
    assert_eq!(vertex_count(&scene, id), 4);
    let mesh = scene.world.mesh(id).unwrap();
    assert!(mesh.revision > replaced);
    assert_eq!(mesh.revision, scene.meshes["piece"].revision);
    drop(mesh);
    scene.redo();
    assert_eq!(vertex_count(&scene, id), 3);
}

#[test]
fn a_prefab_drawing_a_scene_mesh_is_refused() {
    let (scene, id) = scene_drawing(tetra());
    let path = rusty_test_support::temp::dir().join("piece.prefab");
    let error = crate::prefab::save_prefab(&scene, id, path.to_str().unwrap());
    assert!(error.expect_err("refused").contains("scene mesh \"piece\""));
    assert!(!path.exists());
}

#[test]
fn a_mesh_collider_fits_the_scene_mesh_and_follows_a_replace() {
    let (mut scene, id) = scene_drawing(triangle(1.0));
    let collider = ColliderComponent::from_mesh_bounds(false, Vec3::ZERO, Vec3::ZERO);
    scene.world.set_collider(id, Some(collider));
    scene.set_shared_mesh(id, "piece").unwrap();
    let local_max = |scene: &Scene| match scene.world.collider(id).unwrap().shape {
        ColliderShape::Mesh { local_max, .. } => local_max,
        _ => unreachable!(),
    };
    assert_eq!(local_max(&scene), Vec3::new(1.0, 1.0, 0.0));
    scene.define_mesh("piece", triangle(3.0)).unwrap();
    assert_eq!(local_max(&scene), Vec3::new(3.0, 3.0, 0.0));
}

#[test]
fn an_entity_naming_a_missing_mesh_loads_empty() {
    let (scene, id) = scene_drawing(tetra());
    let mut data = to_scene_data(&scene);
    data.meshes.clear();
    let mut loaded = Scene::new();
    apply_scene_data(&mut loaded, data);
    assert_eq!(vertex_count(&loaded, id), 0);
    assert!(crate::rehydrate::warned_of("Scene::piece"));
}

#[test]
fn set_shared_mesh_switches_back_to_a_primitive_keeping_the_settings() {
    let (mut scene, id) = scene_drawing(tetra());
    scene.world.mesh_mut(id).unwrap().max_draw_distance = 12.0;
    scene.set_shared_mesh(id, "sphere").unwrap();
    let mesh = scene.world.mesh(id).unwrap();
    assert_eq!(
        (mesh.primitive_type.as_str(), mesh.asset_ref.as_deref()),
        ("Sphere", None)
    );
    assert_eq!(mesh.max_draw_distance, 12.0);
    assert!(!mesh.vertices.is_empty());
}
