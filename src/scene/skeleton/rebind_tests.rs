//! A re-import re-binds a skeleton's bones in place, by name (#1265): a bone the
//! new rig still names keeps its entity, its flags and the selection.

use std::path::Path;

use glam::Vec3;

use super::fixture::animated_model;
use crate::asset::fixtures_anim::{animated_buffer, ANIMATED_GLTF};
use crate::core::fs::written;
use crate::scene::asset_instance::instantiate_asset;
use crate::scene::model_writes::ModelWrites;
use crate::scene::Scene;

/// Instantiate the animated fixture written under `tag`: the entity, and the
/// model file's path.
fn hero(scene: &mut Scene, tag: &str) -> (u32, String) {
    let model = animated_model(tag);
    let reference = model.to_string_ref();
    let id = instantiate_asset(scene, &reference, Some("Hero"), Vec3::ZERO).unwrap();
    (id, model.path)
}

fn bones(scene: &Scene, owner: u32) -> Vec<u32> {
    scene.world.mesh(owner).unwrap().skeleton.ids()
}

#[test]
fn a_reimport_that_changes_only_vertices_keeps_every_bone_and_its_flags() {
    let mut scene = Scene::new();
    let (hero, path) = hero(&mut scene, "rebind_vertices");
    let old = bones(&scene, hero);
    scene.world.set_active(old[1], false);
    scene.world.set_layer(old[0], 5);
    scene.selected_entity_id = Some(old[1]);
    let first = scene.world.mesh(hero).unwrap().vertices[1].position;
    let mut writes = ModelWrites::default();

    // The buffer's first floats are the positions: move the second vertex.
    let mut buffer = animated_buffer();
    buffer[12..16].copy_from_slice(&3.0f32.to_le_bytes());
    let dir = Path::new(&path).parent().unwrap();
    std::fs::write(dir.join("anim.bin"), buffer).unwrap();
    written::write(Path::new(&path), ANIMATED_GLTF.as_bytes()).unwrap();
    assert_eq!(writes.follow(&mut scene), 1);

    let moved = scene.world.mesh(hero).unwrap().vertices[1].position;
    assert_ne!(moved, first, "the new vertices");
    assert_eq!(bones(&scene, hero), old, "every bone keeps its id");
    assert!(!scene.world.is_active_self(old[1]), "and its active flag");
    assert_eq!(scene.world.layer(old[0]), 5, "and its layer");
    assert_eq!(
        scene.selected_entity_id,
        Some(old[1]),
        "the selection holds"
    );
    assert_eq!(scene.bone_ids().len(), 2, "no bone was added");
}

#[test]
fn a_reimport_that_renames_a_bone_keeps_the_others_and_spawns_the_new_one() {
    let mut scene = Scene::new();
    let (hero, path) = hero(&mut scene, "rebind_renamed");
    let old = bones(&scene, hero);
    scene.selected_entity_id = Some(old[0]);
    let gun = scene.add_entity("Gun".to_string());
    scene.set_parent(gun, Some(old[1])).unwrap();
    let mut writes = ModelWrites::default();

    // `Joint1` leaves the rig and `Joint2` joins it, in the same slot.
    let text = ANIMATED_GLTF.replace(r#""name": "Joint1""#, r#""name": "Joint2""#);
    written::write(Path::new(&path), text.as_bytes()).unwrap();
    assert_eq!(writes.follow(&mut scene), 1);

    let new = bones(&scene, hero);
    assert_eq!(new[0], old[0], "Joint0 survives");
    assert_ne!(new[1], old[1], "Joint2 is a new bone");
    assert_eq!(scene.find_bone(hero, "Joint2"), Some(new[1]));
    assert!(!scene.world.contains(old[1]), "Joint1 is gone");
    assert_eq!(scene.world.parent_id(new[1]), Some(new[0]));
    assert_eq!(
        scene.world.parent_id(gun),
        Some(old[0]),
        "the gun falls back"
    );
    assert_eq!(
        scene.selected_entity_id,
        Some(old[0]),
        "the selection holds"
    );
    assert_eq!(scene.bone_ids().len(), 2);
}
