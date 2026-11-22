use std::collections::BTreeSet;

use super::make_room;
use crate::to_scene_data;
use rusty_components::{LightComponent, LightMode, LightType};
use rusty_scene::create::default_light;
use rusty_scene::lighting::MixedLightingMode;
use rusty_scene::Scene;

fn level_with_deleted_lights() -> Scene {
    let mut level = Scene::new();
    level.lighting_settings.mixed_lighting_mode = MixedLightingMode::Shadowmask;
    for _ in 1..=8 {
        level.add_entity("Prop".to_string());
    }
    level.destroy_entity(7);
    level.destroy_entity(8);
    let set = &mut *level.lightmaps;
    set.lights.extend([7, 8]);
    set.channels.insert(7, 1);
    set.fell_back.insert(8);
    level
}

#[test]
fn a_survivor_holding_a_deleted_lights_id_draws_as_a_real_time_light() {
    let mut s = Scene::new();
    let light = LightComponent {
        light_type: LightType::Point,
        mode: LightMode::Mixed,
        cast_shadows: true,
        ..default_light()
    };
    for _ in 1..=8 {
        let id = s.add_entity("Lamp".to_string());
        s.world.set_light(id, Some(light.clone()));
    }
    s.set_parent(8, Some(7)).unwrap();
    s.dont_destroy_on_load(7);
    let survivors = s.load_survivors();
    assert_eq!(survivors, BTreeSet::from([7, 8]), "no level entity moves");
    let level = to_scene_data(&level_with_deleted_lights());
    crate::load::swap_in(&mut s, level, "rusty_2496_no_such_dir/l.scene", &survivors).unwrap();

    for id in [7, 8] {
        assert!(!s.lightmaps.baked_light(id), "{id}: not in `lights`");
        assert_eq!(s.mask_channel(id), None, "{id}: no channel");
        assert_eq!(
            s.light_mode(id, &light),
            LightMode::Mixed,
            "{id}: real-time"
        );
    }
}

#[test]
fn a_renumbering_that_would_merge_two_baked_ids_is_refused() {
    let mut level = Scene::new();
    level.add_entity("Lamp".to_string());
    let mut data = to_scene_data(&level);
    // Malformed: a baked id (2) at the document's next id, where 1 moves to.
    assert_eq!(data.next_entity_id, 2);
    data.lightmaps.lights.extend([1, 2]);
    make_room(&mut data, &BTreeSet::from([1]));
    assert_eq!(data.entities[0].id, 2);
    assert!(
        data.lightmaps.lights.is_empty(),
        "the set is dropped, not merged"
    );
}
