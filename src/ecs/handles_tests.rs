//! The dense id → handle table (#696): bind, rebind, unbind, gaps and clear.

use super::Handles;

fn two_handles() -> (hecs::Entity, hecs::Entity) {
    let mut w = hecs::World::new();
    (w.spawn(()), w.spawn(()))
}

#[test]
fn binds_rebinds_and_unbinds_by_id() {
    let (a, b) = two_handles();
    let mut h = Handles::default();
    assert_eq!(h.get(1), None, "an empty table binds nothing");
    h.insert(1, a);
    assert_eq!(h.get(1), Some(a));
    h.insert(1, b);
    assert_eq!(h.get(1), Some(b), "a rebind replaces the handle");
    assert_eq!(h.remove(1), Some(b));
    assert_eq!(h.get(1), None);
    assert_eq!(h.remove(1), None, "removing twice is a no-op");
}

#[test]
fn ids_past_a_gap_bind_and_the_gap_stays_unbound() {
    let (a, b) = two_handles();
    let mut h = Handles::default();
    h.insert(2, a);
    h.insert(500, b);
    assert_eq!((h.get(2), h.get(500)), (Some(a), Some(b)));
    assert!((0..500).filter(|&id| id != 2).all(|id| h.get(id).is_none()));
    assert_eq!(h.get(501), None, "past the end is unbound, not a panic");
    assert_eq!(h.get(u32::MAX), None);
    assert_eq!(h.remove(9_999), None);
}

#[test]
fn clear_unbinds_every_id() {
    let (a, b) = two_handles();
    let mut h = Handles::default();
    h.insert(1, a);
    h.insert(3, b);
    h.clear();
    assert_eq!((h.get(1), h.get(3)), (None, None));
    h.insert(1, b);
    assert_eq!(h.get(1), Some(b), "the table refills after a clear");
}
