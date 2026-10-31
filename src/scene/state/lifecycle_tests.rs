//! Every field through every lifecycle, against its promise (#1649). Each mark is
//! set on the base scene, the lifecycle runs (`runners`), and what came back is
//! read as an [`Outcome`]; `check::disagreements` lists every one that breaks
//! its promise. A field with no mark fails, so none is left out.

use super::check::{disagreements, Observed};
use super::inventory::fields;
use super::marks::{all, Mark};
use super::runners::{run, IN_PLAY};
use super::{Field, Lifecycle};

fn field_of(mark: &Mark) -> Field {
    let field = fields().into_iter().find(|f| f.name == mark.field());
    field.unwrap_or_else(|| panic!("{} marks no field", mark.name))
}

#[test]
fn every_field_has_a_mark() {
    let marked: Vec<&str> = all().iter().map(|m| m.field()).collect();
    let bare: Vec<&str> = fields()
        .iter()
        .map(|f| f.name)
        .filter(|n| !marked.contains(n))
        .collect();
    assert!(bare.is_empty(), "fields with no mark in `marks`: {bare:?}");
}

#[test]
fn every_mark_said_to_enter_play_is_a_mark() {
    let names: Vec<&str> = all().iter().map(|m| m.name).collect();
    for name in IN_PLAY {
        assert!(
            names.contains(name),
            "IN_PLAY names `{name}`, which no mark is"
        );
    }
}

#[test]
fn every_lifecycle_keeps_every_class_promise() {
    let mut observed = Vec::new();
    for mark in all() {
        let field = field_of(mark);
        for lifecycle in Lifecycle::ALL {
            let Some(outcome) = run(lifecycle, mark) else {
                continue;
            };
            observed.push(Observed {
                mark: mark.name,
                class: mark.part.unwrap_or(field.class),
                keyed: field.keyed,
                lifecycle,
                outcome,
            });
        }
    }
    assert_eq!(disagreements(&observed), Vec::<String>::new());
}
