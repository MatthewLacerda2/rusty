use glam::Vec3;

use super::{FieldChange, SetStamp};
use crate::lighting::probe::{ProbeField, ProbeVolume};

fn set(xs: &[f32]) -> ProbeVolume {
    let mut v = ProbeVolume::new();
    for &x in xs {
        v.add_probe(Vec3::X * x);
    }
    v
}

fn stamps(sets: &[(u32, &ProbeVolume)]) -> Vec<SetStamp<crate::revision::Revision>> {
    sets.iter()
        .map(|&(h, v)| SetStamp::of(h, v.revision(), v))
        .collect()
}

#[test]
fn an_unchanged_field_is_no_change() {
    let a = set(&[0.0, 1.0]);
    let s = stamps(&[(0, &a)]);
    assert_eq!(FieldChange::between(&s, &s), None);
}

#[test]
fn an_unload_reaches_only_what_stood_near_the_set() {
    let a = set(&[0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
    let b = set(&[100.0, 101.0]);
    let before = stamps(&[(0, &a), (1, &b)]);
    let after = stamps(&[(0, &a)]);
    let change = FieldChange::between(&before, &after).expect("b left");
    assert!(change.touches(1) && !change.touches(0));
    let after_sets = [(0, &a)];
    let field = ProbeField::new(&after_sets);
    assert!(!change.reaches(&field, Vec3::X * 2.0, 0.0), "far from b");
    assert!(
        change.reaches(&field, Vec3::X * 95.0, 0.0),
        "nearer b than a's fifth probe"
    );
    assert!(
        change.reaches(&field, Vec3::X * 2.0, 50.0),
        "a box stretching towards b"
    );
}

#[test]
fn a_set_moved_in_place_reaches_both_boxes_and_a_lone_set_everything() {
    let (a, old, new) = (set(&[0.0; 6]), set(&[50.0]), set(&[-50.0]));
    let before = [SetStamp::of(0, 1, &a), SetStamp::of(1, 1, &old)];
    let after = [SetStamp::of(0, 1, &a), SetStamp::of(1, 2, &new)];
    let change = FieldChange::between(&before, &after).unwrap();
    let sets = [(0, &a), (1, &new)];
    let field = ProbeField::new(&sets);
    assert!(
        !change.reaches(&field, Vec3::X * 0.5, 0.0),
        "a's six probes stand nearer"
    );
    assert!(change.reaches(&field, Vec3::X * 45.0, 0.0), "where it was");
    assert!(
        change.reaches(&field, Vec3::X * -45.0, 0.0),
        "where it went"
    );
    let alone = [(1, &new)];
    let field = ProbeField::new(&alone);
    assert!(
        change.reaches(&field, Vec3::X * 1.0e4, 0.0),
        "no other set: unbounded reach"
    );
    assert!(
        change.reaches(&field, Vec3::NAN, 0.0),
        "a NaN point re-samples"
    );
}
