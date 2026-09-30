//! Scene-side tests for runtime scene loading (#432): which entities survive, how
//! the incoming document makes room for them, and what the swap carries.

use std::collections::BTreeSet;

use super::load::make_room;
use crate::scene::{to_scene_data, Scene};

/// A path with no lighting sidecar beside it: the swap's sidecar merge is a no-op.
const NO_SIDECAR: &str = "rusty_432_no_such_dir/level.scene";

/// An incoming level whose ids (1, 2) collide with the outgoing scene's.
fn level() -> Scene {
    let mut level = Scene::new();
    let floor = level.add_entity("Floor".to_string());
    let lamp = level.add_entity("Lamp".to_string());
    level.set_parent(lamp, Some(floor)).unwrap();
    level
}

#[test]
fn survivors_are_marked_entities_and_their_descendants() {
    let mut s = Scene::new();
    let root = s.add_entity("Music".to_string());
    let child = s.add_entity("Speaker".to_string());
    let other = s.add_entity("Enemy".to_string());
    s.set_parent(child, Some(root)).unwrap();
    assert!(s.dont_destroy_on_load(root));
    assert!(!s.dont_destroy_on_load(99), "no entity, no mark");
    assert_eq!(s.load_survivors(), BTreeSet::from([root, child]));

    assert!(!s.load_survivors().contains(&other));

    s.destroy_entity(root);
    assert!(
        s.load_survivors().is_empty(),
        "a destroyed mark carries nothing"
    );
}

#[test]
fn make_room_moves_colliding_ids_and_rewrites_references() {
    let mut data = to_scene_data(&level()); // Floor = 1, Lamp = 2 (child of 1)
    make_room(&mut data, &BTreeSet::from([1, 5]));
    let floor = &data.entities[0];
    let lamp = &data.entities[1];
    assert_eq!(floor.id, 6, "past both the document and the survivors");
    assert_eq!(lamp.id, 2, "a free id is untouched");
    assert_eq!(
        lamp.parent_id,
        Some(6),
        "the child follows its moved parent"
    );
    assert_eq!(floor.children, vec![2]);
    assert_eq!(data.next_entity_id, 7);
}

#[test]
fn swap_carries_survivors_with_their_ids() {
    let mut s = Scene::new();
    let holder = s.add_entity("Holder".to_string()); // 1, does not survive
    let music = s.add_entity("Music".to_string()); // 2
    let speaker = s.add_entity("Speaker".to_string()); // 3
    s.set_parent(music, Some(holder)).unwrap();
    s.set_parent(speaker, Some(music)).unwrap();
    s.dont_destroy_on_load(music);
    s.request_destroy(holder);
    s.request_destroy(speaker);
    s.spawn_decal(
        glam::Vec3::ZERO,
        glam::Vec3::Y,
        1.0,
        1.0,
        0.0,
        [1.0; 4],
        None,
    );
    let before = s.id();

    let survivors = s.load_survivors();
    s.swap_in(to_scene_data(&level()), NO_SIDECAR, &survivors)
        .unwrap();

    let names: Vec<String> = s
        .entity_ids()
        .into_iter()
        .map(|id| s.world.name(id).unwrap().clone())
        .collect();
    assert_eq!(names, ["Floor", "Lamp", "Music", "Speaker"]);
    assert_eq!(s.find_entity_by_name("Music"), Some(music), "ids kept");
    assert_eq!(s.find_entity_by_name("Speaker"), Some(speaker));
    let floor = s.find_entity_by_name("Floor").unwrap();
    assert!(floor != music && floor != speaker, "incoming ids made room");
    assert_eq!(s.world.parent_id(music), None, "its parent stayed behind");
    assert_eq!(s.world.parent_id(speaker), Some(music));
    assert_eq!(s.persistent, BTreeSet::from([music]), "still marked");
    assert_eq!(
        s.take_pending_destroys(),
        vec![speaker],
        "only survivors' queue"
    );
    assert!(s.decals.is_empty(), "the old scene's decals are gone");
    assert_ne!(s.id(), before, "a new scene earns a new identity");
    let fresh = s.add_entity("Spawned".to_string());
    assert!(fresh > speaker, "the allocator runs past every live id");
}
