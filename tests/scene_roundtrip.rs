//! Scene save/load round-trip + edit-mode snapshot/restore (issue #6).

use glam::Vec3;
use rusty::scene::Scene;
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
        e.mesh = Some(rusty::scene::MeshComponent {
            primitive_type: "Box".to_string(),
            asset_ref: None,
            vertices: Vec::new(),
            indices: Vec::new(),
            is_dirty: rusty::scene::DirtyFlag::new(true),
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
fn save_load_preserves_layer_names_and_entity_layer() {
    let mut scene = Scene::new();
    scene.layers.set_name(3, "Enemy");
    let id = scene.add_entity("Grunt".to_string());
    scene.get_entity_mut(id).unwrap().layer = 3;

    let path = tmp("rusty_layers_roundtrip.scene");
    scene.save_to_file(&path).unwrap();

    let mut loaded = Scene::new();
    loaded.load_from_file(&path).unwrap();
    // The registry name and the entity's layer index both survive the round-trip.
    assert_eq!(loaded.layers.name(3), "Enemy");
    assert_eq!(loaded.layers.index_of("Enemy"), Some(3));
    assert_eq!(loaded.get_entity(id).unwrap().layer, 3);
    // Layer 0 stays the fixed Default and cannot be renamed.
    scene.layers.set_name(0, "NotDefault");
    assert_eq!(scene.layers.name(0), "Default");
}

#[test]
fn save_load_preserves_collision_matrix() {
    let mut scene = Scene::new();
    // Default: every pair collides (including a layer with itself).
    assert!(scene.collision_matrix.can_collide(1, 2));
    assert!(scene.collision_matrix.can_collide(0, 0));

    // Unchecking a cell is symmetric.
    scene.collision_matrix.set_collision(1, 2, false);
    assert!(!scene.collision_matrix.can_collide(1, 2));
    assert!(!scene.collision_matrix.can_collide(2, 1));
    // Other pairs are untouched.
    assert!(scene.collision_matrix.can_collide(1, 3));

    let path = tmp("rusty_collision_matrix_roundtrip.scene");
    scene.save_to_file(&path).unwrap();

    let mut loaded = Scene::new();
    loaded.load_from_file(&path).unwrap();
    assert!(!loaded.collision_matrix.can_collide(1, 2));
    assert!(!loaded.collision_matrix.can_collide(2, 1));
    assert!(loaded.collision_matrix.can_collide(1, 3));
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
