//! A runtime level load that carries an entity (#1714): the incoming walls whose
//! ids the carried one holds move, and the level's PVS moves with them, so culling
//! stays on for the level.

use std::sync::Arc;

use super::test_level::corridors;
use super::{bake_occlusion, fingerprint, save_sidecar};
use rusty_scene::{to_scene_data, Scene};

#[test]
fn a_carried_player_keeps_the_next_levels_culling_on() {
    let mut level = corridors();
    let data = bake_occlusion(&level, 2).0.expect("occluders");
    let wall = data.objects.iter().find(|o| o.occluder).expect("a wall");
    let wall_id = wall.entity;
    *level.occlusion = Some(Arc::new(data));
    let dir = rusty_test_support::temp::dir().join("occlusion_carry");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("level.scene").to_string_lossy().into_owned();
    save_sidecar(&level, &path).unwrap();

    // The outgoing scene's player holds the wall's id.
    let mut s = Scene::new();
    let player = loop {
        let id = s.add_entity("Player".into());
        if id == wall_id {
            break id;
        }
    };
    s.dont_destroy_on_load(player);
    let survivors = s.load_survivors();
    s.swap_in(to_scene_data(&level), &path, &survivors).unwrap();

    let pvs = s.occlusion.as_deref().expect("the level's bake loaded");
    assert!(
        pvs.object_of(player).is_none(),
        "the player was never baked"
    );
    for baked in &pvs.objects {
        assert_eq!(
            fingerprint(&s, baked.entity),
            Some(baked.fingerprint),
            "baked entity {} is the mesh it was baked as",
            baked.entity
        );
    }
}
