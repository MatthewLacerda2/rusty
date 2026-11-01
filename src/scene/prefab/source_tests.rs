//! A model instance's node paths (#1637): one per entity, a duplicate sibling name
//! told apart by `[n]`, a node a bone holds by `<bone>>`.

use std::collections::BTreeMap;

use super::{is_model_source, node_paths};
use crate::components::Entity;
use crate::scene::PrefabData;

fn node(id: u32, name: &str, parent: Option<u32>, bone: &[&str]) -> Entity {
    let mut e = Entity::new(id, name.into());
    e.parent_id = parent;
    e.parent_bone = bone.iter().map(|b| b.to_string()).collect();
    e
}

#[test]
fn every_entity_has_one_path_and_duplicates_are_numbered() {
    let prefab = PrefabData {
        root: 0,
        entities: vec![
            node(0, "kit", None, &[]),
            node(1, "Kit", Some(0), &[]),
            node(2, "Cube", Some(1), &[]),
            node(3, "Cube", Some(1), &[]),
            node(4, "Gun", Some(1), &["Hand", "Root"]),
            node(6, "Gun", Some(1), &["HandL", "Root"]),
            node(5, "Orphan", Some(9), &[]),
        ],
        materials: BTreeMap::new(),
    };
    let paths = node_paths(&prefab);
    let at = |id: u32| paths.get(&id).map(String::as_str);
    assert_eq!(at(0), Some(""));
    assert_eq!(at(1), Some("Kit"));
    assert_eq!(at(2), Some("Kit/Cube"));
    assert_eq!(at(3), Some("Kit/Cube[1]"));
    assert_eq!(at(4), Some("Kit/Hand>Gun"), "keyed by the nearest bone");
    assert_eq!(at(6), Some("Kit/HandL>Gun"), "sockets on two hands differ");
    assert_eq!(at(5), None, "a parent outside the prefab gives no path");
}

#[test]
fn a_prefab_is_not_a_model_source() {
    assert!(is_model_source("models/kit.glb"));
    assert!(!is_model_source("prefabs/kit.prefab"));
    assert!(!is_model_source("scenes/a.scene"));
}

#[test]
fn a_missing_or_broken_model_is_an_error_not_a_panic() {
    let dir = crate::test_temp::dir();
    let missing = dir.join("rusty_1637_missing.glb");
    assert!(super::read_source(&missing.to_string_lossy()).is_err());
    let broken = dir.join("rusty_1637_broken.gltf");
    std::fs::write(&broken, "{ \"asset\": ").unwrap();
    assert!(super::read_source(&broken.to_string_lossy()).is_err());
}
