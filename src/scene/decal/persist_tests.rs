//! Decals across a scene load (#1518): which ones `Scene::swap_in` carries, onto
//! what, and with what kept.

use glam::Vec3;

use crate::scene::decal::DecalSpec;
use crate::scene::{to_scene_data, Scene};

/// A path with no lighting sidecar beside it: the swap's sidecar merge is a no-op.
const NO_SIDECAR: &str = "rusty_1518_no_such_dir/level.scene";

/// `Arena` with a `Crate` under it, the crate at `crate_x`; returns (arena, crate).
fn arena(s: &mut Scene, crate_x: f32) -> (u32, u32) {
    let arena = s.add_entity("Arena".into());
    let crate_ = s.add_entity("Crate".into());
    s.set_parent(crate_, Some(arena)).unwrap();
    s.world.transform_mut(crate_).unwrap().position = Vec3::X * crate_x;
    (arena, crate_)
}

fn spawn(s: &mut Scene, owner: Option<u32>, persist: bool) -> u32 {
    let spec = DecalSpec {
        owner,
        persist,
        lifetime: Some(10.0),
        ..DecalSpec::default()
    };
    s.spawn_decal(Vec3::ZERO, Vec3::Y, spec).unwrap()
}

/// Load the arena again, its crate now at `crate_x`, keeping `Player` if marked.
fn reload(s: &mut Scene, crate_x: f32) {
    let mut level = Scene::new();
    arena(&mut level, crate_x);
    let survivors = s.load_survivors();
    s.swap_in(to_scene_data(&level), NO_SIDECAR, &survivors)
        .unwrap();
}

#[test]
fn only_persistent_decals_cross_with_their_id_age_and_order() {
    let mut s = Scene::new();
    arena(&mut s, 0.0);
    let old = spawn(&mut s, None, true);
    spawn(&mut s, None, false);
    let kept = spawn(&mut s, None, true);
    s.decals.tick(2.0, |_| true);

    reload(&mut s, 0.0);
    let ids: Vec<u32> = s.decals.iter().map(|d| d.id).collect();
    assert_eq!(ids, [old, kept], "the rest went with the scene");
    assert!(s
        .decals
        .iter()
        .all(|d| d.age == 2.0 && d.lifetime == Some(10.0)));
    let new = spawn(&mut s, None, false);
    assert!(new > kept, "ids keep counting");
    assert_eq!(s.decals.back().unwrap().id, new, "survivors stay older");
}

#[test]
fn a_surviving_owner_keeps_its_decal() {
    let mut s = Scene::new();
    let player = s.add_entity("Player".into());
    arena(&mut s, 0.0);
    s.dont_destroy_on_load(player);
    let mark = spawn(&mut s, Some(player), true);
    let before = s.decals[0].owner;

    reload(&mut s, 0.0);
    assert_eq!(s.decals.len(), 1);
    assert_eq!((s.decals[0].id, s.decals[0].owner), (mark, before));
}

#[test]
fn a_left_behind_owner_is_found_again_by_its_path() {
    let mut s = Scene::new();
    let player = s.add_entity("Player".into());
    let (_, crate_) = arena(&mut s, 0.0);
    s.dont_destroy_on_load(player);
    spawn(&mut s, Some(crate_), true);
    let local = s.decals[0].owner.unwrap().local;

    reload(&mut s, 7.0);
    let incoming = s.world.find_by_name("Crate").unwrap();
    assert_ne!(incoming, crate_, "the incoming crate made room for Player");
    let owner = s.decals[0].owner.unwrap();
    assert_eq!((owner.entity, owner.local), (incoming, local));
    let pose = s.decal_pose(&s.decals[0]).unwrap();
    assert_eq!(pose.centre(), Vec3::X * 7.0, "on the crate where it is now");
}

#[test]
fn an_owner_with_no_path_in_the_incoming_scene_drops_its_decal() {
    let mut s = Scene::new();
    let enemy = s.add_entity("Enemy".into());
    spawn(&mut s, Some(enemy), true);
    spawn(&mut s, None, true);

    reload(&mut s, 0.0);
    assert_eq!(s.decals.len(), 1);
    assert_eq!(s.decals[0].owner, None, "the world decal stayed");
}

#[test]
fn a_carried_decal_counts_against_its_budgets() {
    let mut s = Scene::new();
    let (_, crate_) = arena(&mut s, 0.0);
    spawn(&mut s, Some(crate_), true);
    reload(&mut s, 0.0);
    let incoming = s.world.find_by_name("Crate").unwrap();
    let limits = s.decals.limits();
    s.decals
        .set_limits(crate::core::settings::decals::DecalLimits {
            max_per_owner: 1,
            ..limits
        });
    spawn(&mut s, Some(incoming), false);
    assert!(
        s.decals[0].retiring,
        "the carried one is the owner's oldest"
    );
}

#[test]
fn clear_can_keep_the_persistent_trail() {
    let mut s = Scene::new();
    let kept = spawn(&mut s, None, true);
    spawn(&mut s, None, false);
    s.clear_decals(true);
    assert_eq!(s.decals.iter().map(|d| d.id).collect::<Vec<_>>(), [kept]);
    s.clear_decals(false);
    assert!(s.decals.is_empty(), "a plain clear drops the trail too");
}
