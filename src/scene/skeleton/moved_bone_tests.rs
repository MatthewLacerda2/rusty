//! A bone a script moved out from under its skinned entity is an ordinary entity
//! to the scene (#1273, #1277): destroying the character leaves it, and a save
//! keeps it, so a load agrees with a live re-import.

use super::fixture::{hero, reload};
use crate::scene::{to_scene_data, Scene};

/// A hero whose `Joint1` a script moved under a `Prop`; returns the hero, the
/// moved bone and the prop.
fn hero_with_moved_bone(scene: &mut Scene, tag: &str) -> (u32, u32, u32) {
    let hero = hero(scene, tag);
    let joint1 = scene.find_bone(hero, "Joint1").unwrap();
    let prop = scene.add_entity("Prop".to_string());
    scene.set_parent(joint1, Some(prop)).unwrap();
    (hero, joint1, prop)
}

#[test]
fn destroying_the_character_leaves_a_bone_moved_under_a_prop() {
    let mut scene = Scene::new();
    let (hero, joint1, prop) = hero_with_moved_bone(&mut scene, "1273_destroy_moved");
    let joint0 = scene.find_bone(hero, "Joint0").unwrap();
    scene.destroy_entity(hero);
    assert!(!scene.world.contains(joint0), "the rig's own bones go");
    assert_eq!(
        scene.world.parent_id(joint1),
        Some(prop),
        "the moved one stays"
    );
}

#[test]
fn a_save_keeps_a_moved_bone_and_a_load_gives_the_rig_a_fresh_one() {
    let mut scene = Scene::new();
    let (_, joint1, prop) = hero_with_moved_bone(&mut scene, "1273_save_moved");
    let data = to_scene_data(&scene);
    let saved = data.entities.iter().find(|e| e.id == joint1);
    assert_eq!(
        saved.and_then(|e| e.parent_id),
        Some(prop),
        "saved as a plain entity"
    );

    let loaded = reload(&scene);
    let hero = loaded.find_entity_by_name("Hero").unwrap();
    let prop = loaded.find_entity_by_name("Prop").unwrap();
    let on_prop = loaded.world.children(prop);
    assert_eq!(on_prop.len(), 1, "the moved bone is back on the prop");
    assert!(!loaded.is_bone(on_prop[0]));
    let fresh = loaded.find_bone(hero, "Joint1").expect("the rig is whole");
    assert_ne!(fresh, on_prop[0]);
}
