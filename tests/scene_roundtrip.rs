//! Scene save/load round-trip + edit-mode snapshot/restore (issue #6).

use glam::Vec3;
use rusty::core::scene::Scene;
use rusty::scene::SceneSnapshot;

fn tmp(name: &str) -> String {
    std::env::temp_dir()
        .join(name)
        .to_string_lossy()
        .into_owned()
}

#[test]
fn save_load_preserves_values_and_rehydrates_mesh() {
    let mut scene = Scene::new();
    scene.skybox_path = "sky.png".to_string();
    let id = scene.add_entity("Box".to_string());
    {
        let mut e = scene.get_entity_mut(id).unwrap();
        e.transform.position = Vec3::new(1.0, 2.0, 3.0);
        e.mesh = Some(rusty::core::scene::MeshComponent {
            primitive_type: "Box".to_string(),
            vertices: Vec::new(),
            indices: Vec::new(),
            is_dirty: rusty::core::scene::DirtyFlag::new(true),
        });
    }
    let path = tmp("rusty_scene_roundtrip.scene");
    scene.save_to_file(&path).unwrap();

    let mut loaded = Scene::new();
    loaded.load_from_file(&path).unwrap();
    assert_eq!(loaded.entity_count(), 1);
    assert_eq!(loaded.skybox_path, "sky.png");
    let e = loaded.get_entity(id).unwrap();
    assert_eq!(e.transform.position, Vec3::new(1.0, 2.0, 3.0));
    // GPU vertex data is never persisted; it is rehydrated from primitive_type.
    assert!(!e.mesh.as_ref().unwrap().vertices.is_empty());
}

#[test]
fn snapshot_restore_discards_mutations() {
    let mut scene = Scene::new();
    let id = scene.add_entity("Player".to_string());
    let snap = SceneSnapshot::capture(&scene);
    scene.get_entity_mut(id).unwrap().transform.position = Vec3::splat(9.0);
    snap.restore(&mut scene);
    assert_eq!(scene.get_entity(id).unwrap().transform.position, Vec3::ZERO);
}
