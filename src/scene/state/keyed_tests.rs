//! The tables keyed by entity id follow their entities (#1649): the despawn door
//! drops every keyed field's entry, and an Undo of the Destroy puts back exactly
//! the ones classified `Keyed::Restored`. Each keyed field's mark is set on the
//! base entity.

use std::collections::BTreeSet;

use super::inventory::fields;
use super::marks::{all, base, Mark};
use super::Keyed;
use crate::scene::Despawn;

/// Every keyed field's mark, with how it is keyed.
fn keyed() -> Vec<(&'static Mark, Keyed)> {
    let keyed: Vec<_> = fields()
        .into_iter()
        .filter(|f| f.keyed != Keyed::No)
        .collect();
    let marks = all().into_iter().filter(|m| m.part.is_none());
    let found: Vec<_> = marks
        .filter_map(|m| Some((m, keyed.iter().find(|f| f.name == m.field())?.keyed)))
        .collect();
    assert_eq!(found.len(), keyed.len(), "every keyed field has a mark");
    found
}

#[test]
fn the_despawn_door_drops_every_keyed_entry() {
    for (mark, _) in keyed() {
        let (mut scene, base) = base();
        (mark.set)(&mut scene, base);
        assert!((mark.is_set)(&scene, base), "{} set", mark.name);
        scene.despawn_batch(&BTreeSet::from([base]), Despawn::EditMode);
        assert!(!(mark.is_set)(&scene, base), "{} dropped", mark.name);
    }
}

#[test]
fn an_undone_destroy_puts_back_only_what_is_restored_with_its_entity() {
    for (mark, keyed) in keyed() {
        let (mut scene, base) = base();
        (mark.set)(&mut scene, base);
        scene.begin_edits();
        scene.destroy_entity(base);
        scene.commit_edits(None);
        scene.undo();
        assert!(scene.world.contains(base), "the entity is back");
        let back = (mark.is_set)(&scene, base);
        assert_eq!(back, keyed == Keyed::Restored, "{}: {keyed:?}", mark.name);
    }
}
