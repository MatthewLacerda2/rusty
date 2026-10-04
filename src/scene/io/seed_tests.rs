//! The default scene seeds through the manifest (#746), which only works if building
//! it twice gives the same bytes; and Reset Scene rebuilds it rather than reloading.

use super::super::scene_json;
use crate::scene::{default_scene, Scene};

fn built_json() -> String {
    let mut scene = Scene::new();
    default_scene::build(&mut scene, default_scene::BOT_SCRIPT);
    scene_json(&scene).expect("the default scene serializes")
}

#[test]
fn the_default_scene_serializes_to_the_same_bytes_every_build() {
    // Otherwise every boot would see a "newer default" and never trust its record.
    assert_eq!(built_json(), built_json());
}

#[test]
fn building_the_default_scene_replaces_what_was_there() {
    let mut scene = Scene::new();
    crate::scene::authoring::create_entity(&mut scene, "Stray", None);
    for _ in 0..20 {
        crate::scene::authoring::create_entity(&mut scene, "Filler", None);
    }
    let path = super::build_default_scene(&mut scene);
    assert_eq!(path, super::DEFAULT_SCENE_PATH);
    // No stray entity, no shifted ids: exactly the freshly built default.
    assert_eq!(scene_json(&scene).unwrap(), built_json());
}
