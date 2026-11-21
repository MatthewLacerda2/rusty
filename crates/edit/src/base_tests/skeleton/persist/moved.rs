//! A Play load that moves a selected generated hitbox off a survivor's id and
//! then removes it (its bone is gone from the model) leaves no selection on the
//! dead id: the selection was renamed with the entities, before the apply.

use std::collections::BTreeSet;

use super::super::fixture::hero;
use crate::serialize::to_scene_data;
use rusty_scene::skeleton::{HitboxOptions, HITBOX_NAME};
use rusty_scene::Scene;

#[test]
fn a_moved_selected_hitbox_removed_on_load_leaves_no_selection() {
    let mut level = Scene::new();
    let owner = hero(&mut level, "moved_hitbox");
    let opts = HitboxOptions {
        min_size: 0.0,
        ..HitboxOptions::default()
    };
    let made = level.generate_hitboxes(owner, &opts).unwrap();
    let hitbox = made[0].1;
    let mut data = to_scene_data(&level);
    for doc in data.entities.iter_mut().filter(|e| e.name == HITBOX_NAME) {
        doc.parent_bone = vec!["NoSuchBone".to_string()];
    }
    data.selected_entity_id = Some(hitbox);

    // A carried player whose parts hold every id the level uses.
    let mut s = Scene::new();
    let player = s.add_entity("Player".to_string());
    s.dont_destroy_on_load(player);
    let last = data.entities.iter().map(|e| e.id).max().unwrap();
    while s.load_survivors().len() <= last as usize {
        let part = s.add_entity("Part".to_string());
        s.set_parent(part, Some(player)).unwrap();
    }
    let survivors: BTreeSet<u32> = s.load_survivors();
    assert!(survivors.contains(&hitbox), "the hitbox moves");
    crate::load::swap_in(&mut s, data, "rusty_2496_no_such_dir/l.scene", &survivors).unwrap();

    let named = |name: &str| {
        s.world
            .ids()
            .iter()
            .any(|&id| s.world.name(id).is_some_and(|n| *n == name))
    };
    assert!(
        !named(HITBOX_NAME),
        "the hitboxes whose bone is gone were removed"
    );
    assert_eq!(s.selected_entity_id, None, "and the selection with it");
}
