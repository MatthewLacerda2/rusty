//! A Play load's dead lightmap entries go before `make_room` moves an id onto them.

use std::collections::BTreeSet;

use crate::to_scene_data;
use rusty_scene::lighting::lightmap_set::LightmapEntry;
use rusty_scene::Scene;

const NO_SIDECAR: &str = "rusty_2617_no_such_dir/level.scene";

fn entry(entity: u32, u: f32) -> LightmapEntry {
    LightmapEntry {
        entity,
        page: 0,
        scale_offset: [0.5, 0.5, u, 0.0],
    }
}

#[test]
fn a_moved_entity_never_lands_on_a_dead_entrys_lightmap() {
    let mut level = Scene::new();
    let crate_id = level.add_entity("Crate".to_string());
    let wall = level.add_entity("Wall".to_string());
    let mut data = to_scene_data(&level);
    let dead = data.next_entity_id;
    // The wall's own entry, and one for an entity the file does not hold, at the next free id.
    data.lightmaps.insert(entry(wall, 0.25));
    data.lightmaps.insert(entry(dead, 0.75));

    let mut s = Scene::new();
    let player = s.add_entity("Player".to_string());
    s.dont_destroy_on_load(player);
    let survivors = s.load_survivors();
    assert_eq!(
        survivors,
        BTreeSet::from([crate_id]),
        "the crate's id is taken"
    );
    crate::load::swap_in(&mut s, data, NO_SIDECAR, &survivors).unwrap();

    let moved = s.find_entity_by_name("Crate").unwrap();
    assert_eq!(moved, dead, "the crate moves onto the dead entry's id");
    assert_eq!(s.lightmaps.get(moved), None, "no lightmap it did not own");
    let wall = s.find_entity_by_name("Wall").unwrap();
    assert_eq!(
        s.lightmaps.get(wall),
        Some(&entry(wall, 0.25)),
        "the level still draws"
    );
    assert_eq!(s.lightmaps.get(player), None);
}
