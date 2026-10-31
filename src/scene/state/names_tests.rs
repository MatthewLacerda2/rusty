//! The inventory against the lists it governs (#1649): the scene document's
//! fields, and the gates' own flag and pass cases.

use super::check::{disagreements, mismatched, Observed};
use super::inventory::fields;
use super::{Class, Keyed, Lifecycle, Outcome};
use crate::scene::authoring::fields::introspect::serde_fields;
use crate::scene::SceneData;

#[test]
fn the_scene_document_holds_exactly_the_saved_classes() {
    let names = serde_fields::<SceneData>().expect("SceneData reads as a struct");
    let saved: Vec<&str> = fields().iter().flat_map(|f| f.document_names()).collect();
    assert_eq!(mismatched("SceneData", names, &saved), Vec::<String>::new());
}

#[test]
fn a_field_classified_for_nothing_and_one_never_classified_are_flagged() {
    let flagged = mismatched("SceneData", &["fog", "new_knob"], &["fog", "gone"]);
    assert_eq!(flagged.len(), 2, "{flagged:?}");
    assert!(
        flagged[0].contains("`new_knob` is not classified"),
        "{flagged:?}"
    );
    assert!(flagged[1].contains("`gone`"), "{flagged:?}");
    assert!(mismatched("SceneData", &["fog"], &["fog"]).is_empty());
}

#[test]
fn a_lifecycle_breaking_its_class_is_flagged_and_one_keeping_it_passes() {
    let seen = |class, lifecycle, outcome| Observed {
        mark: "fog",
        class,
        keyed: Keyed::No,
        lifecycle,
        outcome,
    };
    let forgotten = [seen(Class::Authored, Lifecycle::Undo, Outcome::Left)];
    let flagged = disagreements(&forgotten);
    assert_eq!(flagged.len(), 1);
    assert!(
        flagged[0].contains("Undo owes Authored (No) Back"),
        "{flagged:?}"
    );
    let kept = [
        seen(Class::Authored, Lifecycle::Undo, Outcome::Back),
        seen(Class::BakeOutput, Lifecycle::Undo, Outcome::Left),
        // Owed nothing, so whatever happened passes.
        seen(Class::Follow, Lifecycle::Stop, Outcome::Left),
    ];
    assert!(disagreements(&kept).is_empty());
}

/// A field keyed by entity that a Play load carries across, whatever its class
/// allows, is flagged: the old id may name an incoming entity.
#[test]
fn a_play_load_keeping_a_keyed_entry_is_flagged() {
    let seen = |keyed| Observed {
        mark: "pending_destroy",
        class: Class::Runtime,
        keyed,
        lifecycle: Lifecycle::PlayLoad,
        outcome: Outcome::Left,
    };
    assert_eq!(disagreements(&[seen(Keyed::Dropped)]).len(), 1);
    assert!(disagreements(&[seen(Keyed::No)]).is_empty());
}
