//! The conformance roster names every [`ColliderShape`] variant (#1171): the
//! exhaustive `match` in [`super::samples`] makes a new variant write samples, and
//! this makes [`KINDS`] call it.

use std::collections::BTreeSet;

use super::KINDS;
use crate::components::ColliderShape;

#[test]
fn the_roster_names_every_variant() {
    // serde lists every variant when it meets an unknown one.
    let err = serde_json::from_str::<ColliderShape>("\"?\"")
        .unwrap_err()
        .to_string();
    let names: BTreeSet<String> = KINDS
        .iter()
        .map(|k| serde_json::to_value(k).expect("serialises"))
        .filter_map(|v| v.as_object()?.keys().next().cloned())
        .collect();
    let listed = err.matches('`').count() / 2 - 1;
    assert_eq!(
        names.len(),
        listed,
        "{err}: KINDS lacks a variant ({names:?})"
    );
    assert!(
        names.iter().all(|n| err.contains(&format!("`{n}`"))),
        "{err}"
    );
}
