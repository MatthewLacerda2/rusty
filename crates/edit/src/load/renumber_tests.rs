//! A load that moves incoming ids out of the survivors' way moves the level's
//! baked lightmaps with them: the moved entity keeps its lightmap, its baked light
//! and its shadowmask channel, and the survivor that kept the old id gains none.

use std::collections::BTreeSet;

use crate::to_scene_data;
use rusty_scene::lighting::lightmap_set::LightmapEntry;
use rusty_scene::Scene;

/// A path with no lighting sidecar beside it: the swap's sidecar merge is a no-op.
const NO_SIDECAR: &str = "rusty_2496_no_such_dir/level.scene";

/// A level whose wall (1) is lightmapped and whose lamp (2) the bake took in,
/// shadowmasked on channel 3; a second lamp (3) fell back for want of a channel.
fn lit_level() -> Scene {
    let mut level = Scene::new();
    let wall = level.add_entity("Wall".to_string());
    let lamp = level.add_entity("Lamp".to_string());
    let spare = level.add_entity("Spare".to_string());
    level.lightmaps.pages = vec!["page0.ktx2".into()];
    level.lightmaps.insert(LightmapEntry {
        entity: wall,
        page: 0,
        scale_offset: [0.5, 0.5, 0.25, 0.0],
    });
    level.lightmaps.lights.extend([lamp, spare]);
    level.lightmaps.channels.insert(lamp, 3);
    level.lightmaps.fell_back.insert(spare);
    level
}

/// A playing scene whose carried player (1) and its gun (2) and sight (3) hold
/// every id of the level's.
fn carrying_a_player() -> (Scene, BTreeSet<u32>) {
    let mut s = Scene::new();
    let player = s.add_entity("Player".to_string());
    for name in ["Gun", "Sight"] {
        let part = s.add_entity(name.to_string());
        s.set_parent(part, Some(player)).unwrap();
    }
    s.dont_destroy_on_load(player);
    let survivors = s.load_survivors();
    assert_eq!(survivors, BTreeSet::from([1, 2, 3]));
    (s, survivors)
}

#[test]
fn a_moved_entity_keeps_its_baked_lighting_and_a_survivor_gains_none() {
    let (mut s, survivors) = carrying_a_player();
    crate::load::swap_in(&mut s, to_scene_data(&lit_level()), NO_SIDECAR, &survivors).unwrap();

    let id = |name: &str| s.find_entity_by_name(name).unwrap();
    let (wall, lamp, spare) = (id("Wall"), id("Lamp"), id("Spare"));
    assert!(
        !survivors.contains(&wall),
        "the wall moved off the player's id"
    );
    let entry = s.lightmaps.get(wall).expect("the wall keeps its lightmap");
    assert_eq!(entry.scale_offset, [0.5, 0.5, 0.25, 0.0]);
    assert!(s.lightmaps.baked_light(lamp) && s.lightmaps.baked_light(spare));
    assert_eq!(s.lightmaps.mask_channel(lamp), Some(3));
    assert!(s.lightmaps.fell_back.contains(&spare));
    for survivor in survivors {
        assert!(
            s.lightmaps.get(survivor).is_none(),
            "{survivor}: no lightmap"
        );
        assert!(
            !s.lightmaps.baked_light(survivor),
            "{survivor}: no baked light"
        );
        assert_eq!(s.lightmaps.mask_channel(survivor), None, "{survivor}");
        assert!(!s.lightmaps.fell_back.contains(&survivor), "{survivor}");
    }
}

#[test]
fn a_load_with_no_survivors_keeps_the_lightmaps_as_saved() {
    let level = lit_level();
    let mut s = Scene::new();
    s.add_entity("Leftover".to_string());
    crate::load::swap_in(&mut s, to_scene_data(&level), NO_SIDECAR, &BTreeSet::new()).unwrap();
    assert_eq!(*s.lightmaps, *level.lightmaps);
}
