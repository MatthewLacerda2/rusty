//! crates/physics/src/shape/decompose/panic_tests.rs — a panic inside V-HACD (quickhull on
//! a degenerate imported prop) is no decomposition, not a crash (#1886).

use glam::Vec3;

use super::PANIC_NEXT;
use crate::shape::build_tests::mesh_shape;
use crate::shape::fit::build_shape_for;
use crate::shape::fixture::arch;

#[test]
fn a_panicking_decomposition_keeps_the_trimesh_and_warns_once() {
    // The arch moved where no other test puts it, so its cache key is this test's own.
    let scale = Vec3::new(1.31, 0.77, 1.13);
    let (mut positions, indices) = arch();
    positions.iter_mut().for_each(|p| p[2] += 93.0);
    rusty_test_support::logs::init();
    rusty_test_support::logs::take();
    PANIC_NEXT.with(|p| p.set(true));
    let geom = Some((positions.as_slice(), indices.as_slice()));
    let built = build_shape_for(9300, &mesh_shape(false), scale, geom, true);
    assert!(!PANIC_NEXT.with(|p| p.get()), "the seam fired");
    let built = built.expect("a collider, not a skip");
    assert!(
        built.shape().as_trimesh().is_some(),
        "falls back to the trimesh"
    );
    let logged = rusty_test_support::logs::take();
    let named = logged.iter().filter(|m| m.contains("entity 9300:")).count();
    assert_eq!(named, 1, "{logged:?}");
    // The failure is kept: a second copy is not decomposed again (it would
    // succeed now, the seam spent) but reuses the kept `None`.
    let again = build_shape_for(9301, &mesh_shape(false), scale, geom, true).unwrap();
    assert!(
        again.shape().as_trimesh().is_some(),
        "a cache hit, not a rerun"
    );
}
