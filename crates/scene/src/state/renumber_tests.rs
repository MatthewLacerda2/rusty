//! A load that moves incoming ids out of the survivors' way renames them in every
//! saved field keyed by entity (`runtime::load::RENUMBERED`): the inventory names
//! which fields those are, so a new one that `Scene::renumber` skips fails here.

use super::inventory::fields;
use super::{Class, Field, Keyed};
use crate::lighting::lightmap_set::LightmapEntry;
use crate::runtime::load::{Moves, RENUMBERED};
use crate::Scene;

/// What disagrees between the fields a load reads entity ids into (saved and
/// keyed by entity) and the ones `renumbered` follows.
fn disagreements(fields: &[Field], renumbered: &[&str]) -> Vec<String> {
    let owed: Vec<&str> = fields
        .iter()
        .filter(|f| f.class.saved() && f.keyed != Keyed::No)
        .map(|f| f.name)
        .collect();
    let missing = owed.iter().filter(|name| !renumbered.contains(name));
    let missing = missing
        .map(|name| format!("`{name}` is saved and keyed by entity: add it to `RENUMBERED`"));
    let extra = renumbered.iter().filter(|name| !owed.contains(name));
    let extra = extra.map(|name| format!("`{name}` is renumbered but no saved keyed field"));
    missing.chain(extra).collect()
}

fn renumbered() -> Vec<&'static str> {
    RENUMBERED.iter().map(|(name, _)| *name).collect()
}

#[test]
fn every_saved_field_keyed_by_entity_follows_a_renumbering() {
    assert_eq!(
        disagreements(&fields(), &renumbered()),
        Vec::<String>::new()
    );
}

#[test]
fn a_keyed_field_the_renumbering_skips_is_flagged_and_an_unkeyed_one_passes() {
    let field = |name, keyed| Field {
        name,
        class: Class::BakeOutput,
        keyed,
        saved_as: &[],
    };
    let mut mocked = fields();
    mocked.push(field("mock_bake", Keyed::Dropped));
    let flagged = disagreements(&mocked, &renumbered());
    assert_eq!(flagged.len(), 1, "{flagged:?}");
    assert!(flagged[0].contains("`mock_bake`"), "{flagged:?}");
    let mut unkeyed = fields();
    unkeyed.push(field("mock_settings", Keyed::No));
    assert!(disagreements(&unkeyed, &renumbered()).is_empty());
    let mut stale = renumbered();
    stale.push("gone");
    assert_eq!(disagreements(&fields(), &stale).len(), 1);
}

#[test]
fn a_renumbering_moves_the_selection_and_every_lightmap_key() {
    let mut scene = Scene::new();
    let entry = |entity| LightmapEntry {
        entity,
        page: 0,
        scale_offset: [1.0, 1.0, 0.0, 0.0],
    };
    scene.selected_entity_id = Some(1);
    scene.lightmaps.extend([entry(1), entry(2)]);
    scene.lightmaps.lights.insert(1);
    scene.lightmaps.channels.insert(1, 2);
    scene.lightmaps.fell_back.insert(1);
    let mut hidden = crate::lighting::lightmap_set::LightmapSet::default();
    hidden.insert(entry(1));
    scene.lightmaps.shown_over = Some(Box::new(hidden));
    let before = scene.lightmaps.clone();
    scene.renumber(&Moves::new());
    assert_eq!(scene.lightmaps, before, "no moves, no change");

    scene.renumber(&Moves::from([(1, 9)]));
    let set = &scene.lightmaps;
    assert_eq!(scene.selected_entity_id, Some(9));
    assert_eq!(
        set.get(9).map(|e| e.entity),
        Some(9),
        "the entry names its id"
    );
    assert!(set.get(1).is_none() && set.get(2).is_some(), "only 1 moved");
    assert!(set.baked_light(9) && !set.baked_light(1));
    assert_eq!((set.mask_channel(9), set.mask_channel(1)), (Some(2), None));
    assert!(set.fell_back.contains(&9) && !set.fell_back.contains(&1));
    assert!(
        set.saved().get(9).is_some(),
        "the lightmaps a preview hides move too"
    );
}
