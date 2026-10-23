//! Which slot draws which mesh run (#1285), Unity's way.

use crate::components::{MaterialAsset, MaterialComponent};
use crate::scene::Scene;

/// An entity referencing `keys`, every one in the library but `"missing"`.
fn entity(keys: &[&str]) -> (Scene, u32) {
    let mut scene = Scene::new();
    let id = scene.add_entity("E".into());
    let materials = keys.iter().map(|k| k.to_string()).collect();
    scene
        .world
        .set_material(id, Some(MaterialComponent { materials }));
    for key in keys.iter().filter(|k| **k != "missing") {
        scene
            .materials
            .insert(key.to_string(), MaterialAsset::default());
    }
    (scene, id)
}

/// `(slot, run, key)` per draw of `id` over `runs` runs.
fn draws(scene: &Scene, id: u32, runs: usize) -> Vec<(usize, usize, Option<&str>)> {
    let draws = scene.material_draws(id, runs);
    draws
        .iter()
        .map(|d| (d.slot, d.run, d.entry.map(|e| e.0)))
        .collect()
}

#[test]
fn each_slot_draws_its_own_run() {
    let (scene, id) = entity(&["red", "blue"]);
    assert_eq!(
        draws(&scene, id, 2),
        [(0, 0, Some("red")), (1, 1, Some("blue"))]
    );
}

#[test]
fn an_extra_slot_draws_the_last_run_again() {
    let (scene, id) = entity(&["red", "blue", "glow"]);
    let last = draws(&scene, id, 2).pop();
    assert_eq!(last, Some((2, 1, Some("glow"))));
}

#[test]
fn a_run_past_the_last_slot_is_not_drawn() {
    let (scene, id) = entity(&["red"]);
    assert_eq!(draws(&scene, id, 3), [(0, 0, Some("red"))]);
}

#[test]
fn a_missing_key_draws_with_the_default_and_no_material_draws_every_run() {
    let (mut scene, id) = entity(&["missing"]);
    assert_eq!(draws(&scene, id, 1), [(0, 0, None)]);
    let bare = scene.add_entity("Bare".into());
    assert_eq!(draws(&scene, bare, 2), [(0, 0, None), (1, 1, None)]);
}
