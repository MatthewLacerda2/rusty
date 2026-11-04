//! src/scene/revision_tests.rs — a revision moves at a write and only then.

use super::{Revision, Revisioned};

#[test]
fn a_write_moves_the_revision_and_a_read_or_a_clone_does_not() {
    let mut v = Revisioned::new(vec![1, 2]);
    let at = v.revision();
    assert_eq!(v.len(), 2);
    assert_eq!(v.clone().revision(), at, "a copy keeps its original's");
    assert_eq!(v.revision(), at, "a read leaves it");
    v.push(3);
    assert_ne!(v.revision(), at, "a write moves it");
}

#[test]
fn a_new_a_loaded_and_a_restored_value_each_differ_from_the_one_seen() {
    let mut live = Revisioned::new(vec![1]);
    let snapshot = live.clone();
    live.push(2);
    let seen = live.revision();
    live = snapshot;
    assert_ne!(live.revision(), seen, "a restore reads as a change");
    let loaded: Revisioned<Vec<i32>> = serde_json::from_str("[1, 2]").unwrap();
    assert_ne!(loaded.revision(), seen);
    assert_eq!(
        serde_json::to_string(&loaded).unwrap(),
        "[1,2]",
        "saved as the value"
    );
    assert_eq!(loaded, Revisioned::new(vec![1, 2]), "compared as the value");
    assert_ne!(Revision::default(), Revision::default());
}
