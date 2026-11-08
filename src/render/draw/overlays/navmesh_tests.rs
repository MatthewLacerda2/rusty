use std::cell::Cell;

use super::*;

/// The same revision builds nothing; a new one rebuilds, an empty build keeps none.
#[test]
fn kept_rebuilds_only_when_the_revision_moves() {
    let mut kept: Kept<u32> = Kept::default();
    let builds = Cell::new(0);
    let build = |v| {
        builds.set(builds.get() + 1);
        v
    };
    assert!(kept.refresh(7, || build(Some(1))));
    assert!(!kept.refresh(7, || build(Some(2))), "same revision");
    assert_eq!((kept.get(), builds.get()), (Some(&1), 1));
    assert!(kept.refresh(8, || build(None)));
    assert_eq!(
        (kept.get(), builds.get()),
        (None, 2),
        "an empty navmesh keeps nothing"
    );
    assert!(!kept.refresh(8, || build(Some(3))), "still revision 8");
    assert_eq!(builds.get(), 2);
}

/// A navmesh past the buffer limit is cut at a whole quad, never mid-triangle.
#[test]
fn a_navmesh_past_the_limit_keeps_whole_quads() {
    assert_eq!(vertices_that_fit(12, 1 << 20), 12);
    assert_eq!(vertices_that_fit(600, 12 * 100), 96);
    assert_eq!(vertices_that_fit(600, 12 * 5), 0);
    assert_eq!(vertices_that_fit(0, 0), 0);
}
