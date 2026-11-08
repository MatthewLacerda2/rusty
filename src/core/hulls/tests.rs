//! src/core/hulls/tests.rs — kept decompositions: keyed by content, bounded,
//! and marked for writing only where something writes (#1940). Each test works
//! on a `Store` of its own, so no test reads what another one kept.

use super::*;

fn one(x: f32) -> Pieces {
    let points = vec![[x, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [0.0; 3]];
    Arc::new(vec![Hull {
        points,
        triangles: vec![[0, 1, 2]],
    }])
}

#[test]
fn a_key_follows_every_coordinate_and_index() {
    let p = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
    let base = key(p.iter().copied(), &[0, 1, 2]);
    assert_eq!(base, key(p.iter().copied(), &[0, 1, 2]));
    assert_ne!(base, key(p.iter().copied(), &[0, 2, 1]));
    let mut q = p;
    q[2][1] = 1.0 + f32::EPSILON;
    assert_ne!(base, key(q.iter().copied(), &[0, 1, 2]));
    assert_eq!((base.1, base.2), (3, 3));
}

#[cfg(any(feature = "editor", feature = "dev"))]
#[test]
fn fresh_pieces_are_kept_and_handed_out_once() {
    let mut store = Store::default();
    let k = (1, 4, 3);
    store.keep_fresh(k, one(1.0));
    assert_eq!(store.pieces.get(&k), Some(&one(1.0)));
    assert_eq!(store.take_fresh(), vec![(k, one(1.0))]);
    assert!(store.take_fresh().is_empty(), "handed out once");
    assert_eq!(
        store.pieces.get(&k),
        Some(&one(1.0)),
        "kept after the write"
    );
    store.insert((2, 4, 3), one(2.0));
    assert!(store.take_fresh().is_empty(), "a loaded entry is not fresh");
}

#[test]
fn past_the_bound_the_oldest_goes() {
    let mut store = Store::default();
    for n in 0..=KEPT as u64 {
        store.keep_fresh((n, 0, 0), one(1.0));
    }
    assert!(!store.pieces.contains_key(&(0, 0, 0)));
    assert!(store.pieces.contains_key(&(KEPT as u64, 0, 0)));
    assert_eq!(store.pieces.len(), KEPT);
}
