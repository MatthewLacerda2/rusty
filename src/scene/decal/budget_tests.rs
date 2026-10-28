//! The decal budgets (#1517): per owner, the world and the total, each fading the
//! oldest of its bucket out; blend order stays global spawn order.

use glam::Vec3;

use super::{EVICTION_FADE, EVICTION_HEADROOM};
use crate::core::settings::decals::DecalLimits;
use crate::scene::decal::DecalSpec;
use crate::scene::Scene;

fn limits(max_per_owner: u32, max_world: u32, max_total: u32) -> DecalLimits {
    DecalLimits {
        max_per_owner,
        max_world,
        max_total,
    }
}

fn stamp(s: &mut Scene, owner: Option<u32>, x: f32) -> u32 {
    let spec = DecalSpec {
        owner,
        ..DecalSpec::default()
    };
    s.spawn_decal(Vec3::new(x, 0.0, 0.0), Vec3::Y, spec)
        .unwrap()
}

fn whole(s: &Scene) -> Vec<u32> {
    s.decals
        .iter()
        .filter(|d| !d.retiring)
        .map(|d| d.id)
        .collect()
}

#[test]
fn one_owner_filling_its_budget_never_evicts_another_owners_mark() {
    let mut s = Scene::new();
    let (floor, crate_) = (s.add_entity("Floor".into()), s.add_entity("Crate".into()));
    let mark = stamp(&mut s, Some(crate_), 0.0);
    for i in 0..=256 {
        stamp(&mut s, Some(floor), i as f32);
    }
    let floor_whole = s
        .decals
        .iter()
        .filter(|d| d.bucket() == Some(floor) && !d.retiring);
    assert_eq!(floor_whole.count(), 256, "the floor keeps its own 256");
    assert!(whole(&s).contains(&mark), "the crate's mark stays whole");
    let evicted = s.decals.iter().find(|d| d.retiring).unwrap();
    assert_eq!(evicted.position.x, 0.0, "the floor's oldest fades");
    assert_eq!(evicted.bucket(), Some(floor));
}

#[test]
fn past_a_budget_the_oldest_fades_out_instead_of_popping() {
    let mut s = Scene::new();
    s.decals.set_limits(limits(4, 4, 100));
    let ids: Vec<u32> = (0..5).map(|i| stamp(&mut s, None, i as f32)).collect();
    assert_eq!(s.decals.len(), 5, "nothing popped");
    assert!(s.decals[0].retiring && s.decals[0].opacity() == 1.0);
    s.decals.tick(EVICTION_FADE * 0.5, |_| true);
    assert!((s.decals[0].opacity() - 0.5).abs() < 1e-5);
    s.decals.tick(EVICTION_FADE * 0.5, |_| true);
    assert_eq!(whole(&s), ids[1..], "faded out, then dropped");
}

#[test]
fn past_the_total_the_oldest_in_the_scene_fades_whatever_its_bucket() {
    let mut s = Scene::new();
    s.decals.set_limits(limits(10, 10, 3));
    let owner = s.add_entity("Door".into());
    let first = stamp(&mut s, Some(owner), 0.0);
    let rest = [stamp(&mut s, None, 1.0), stamp(&mut s, None, 2.0)];
    let newest = stamp(&mut s, Some(owner), 3.0);
    assert!(!whole(&s).contains(&first), "the scene's oldest fades");
    assert_eq!(whole(&s), [rest[0], rest[1], newest]);
}

#[test]
fn blend_order_stays_global_spawn_order_across_buckets() {
    let mut s = Scene::new();
    s.decals.set_limits(limits(2, 2, 100));
    let owner = s.add_entity("Crate".into());
    let mut spawned = Vec::new();
    for i in 0..8 {
        let bucket = (i % 2 == 0).then_some(owner);
        spawned.push(stamp(&mut s, bucket, i as f32));
    }
    let ring: Vec<u32> = s.decals.iter().map(|d| d.id).collect();
    assert_eq!(ring, spawned, "oldest first, every bucket interleaved");
    assert_eq!(whole(&s), spawned[4..], "each bucket kept its newest two");
}

#[test]
fn lowering_a_limit_fades_the_oldest_excess() {
    let mut s = Scene::new();
    let ids: Vec<u32> = (0..6).map(|i| stamp(&mut s, None, i as f32)).collect();
    s.decals.set_limits(limits(256, 2, 4096));
    assert_eq!(whole(&s), ids[4..]);
    assert_eq!(s.decals.len(), 6, "faded, not popped");
}

#[test]
fn the_headroom_bounds_the_total_when_fades_cannot_keep_up() {
    let mut s = Scene::new();
    s.decals.set_limits(limits(256, 8, 4096));
    for i in 0..8 + EVICTION_HEADROOM + 10 {
        stamp(&mut s, None, i as f32);
    }
    assert_eq!(s.decals.len(), 8 + EVICTION_HEADROOM);
    assert_eq!(whole(&s).len(), 8);
}
