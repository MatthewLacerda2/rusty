//! The headroom and the running counts (#1517): only a spawn's evictions are
//! bounded by the headroom, and the counts stay in step with the ring.

use super::budget_tests::{limits, stamp, whole};
use super::EVICTION_HEADROOM;
use crate::scene::Scene;

#[test]
fn lowering_a_budget_past_the_headroom_fades_every_excess_decal() {
    let mut s = Scene::new();
    let n = 2 * EVICTION_HEADROOM as u32;
    let ids: Vec<u32> = (0..n + 4).map(|i| stamp(&mut s, None, i as f32)).collect();
    s.decals.set_limits(limits(256, 4, 4096));
    assert_eq!(s.decals.len(), ids.len(), "nothing popped");
    assert_eq!(whole(&s), ids[n as usize..]);
    stamp(&mut s, None, 0.0);
    assert_eq!(
        s.decals.len(),
        ids.len() + 1,
        "a spawn after it pops none either"
    );
}

#[test]
fn scripted_fades_past_the_headroom_all_run_their_course() {
    let mut s = Scene::new();
    let ids: Vec<u32> = (0..200).map(|i| stamp(&mut s, None, i as f32)).collect();
    for &id in &ids {
        assert!(s.decals.remove(id, 2.0));
    }
    stamp(&mut s, None, 0.0);
    assert_eq!(s.decals.len(), 201, "nothing popped");
    assert!(s.decals.iter().take(200).all(|d| d.retiring && !d.evicted));
}

#[test]
fn the_running_counts_match_the_ring_after_adds_fades_and_drops() {
    let mut s = Scene::new();
    s.decals.set_limits(limits(3, 3, 5));
    let (a, b) = (s.add_entity("A".into()), s.add_entity("B".into()));
    let ids: Vec<u32> = (0..12)
        .map(|i| stamp(&mut s, [None, Some(a), Some(b)][i % 3], i as f32))
        .collect();
    s.decals.remove(ids[11], 0.0);
    s.decals.remove(ids[10], 1.0);
    s.decals.tick(0.0, |e| e != b);
    let count = |bucket| {
        whole(&s)
            .iter()
            .filter(|&&id| s.decals.iter().any(|d| d.id == id && d.bucket() == bucket))
            .count() as u32
    };
    let c = &s.decals.counts;
    assert_eq!(
        (c.whole(None), c.whole(Some(a)), c.whole(Some(b))),
        (count(None), count(Some(a)), 0)
    );
    assert_eq!(c.total as usize, whole(&s).len());
    assert_eq!(c.evicting, s.decals.iter().filter(|d| d.evicted).count());
}
