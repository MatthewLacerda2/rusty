//! In Play a stale hitbox is queued, not removed on the spot (#1298), so the
//! destroy phase can tear down any script under it first, and every lookup
//! already treats it as gone.

use super::rig::rig;
use super::*;
use crate::scene::skeleton::RagdollOptions;

#[test]
fn in_play_a_stale_hitbox_is_queued_for_the_destroy_phase() {
    let mut scene = Scene::new();
    let owner = rig(&mut scene);
    scene
        .generate_hitboxes(owner, &HitboxOptions::default())
        .unwrap();
    let head = scene.find_bone(owner, "head").unwrap();
    let stale = scene.hitbox_of(head).unwrap();
    scene.set_playing(true);
    let opts = HitboxOptions {
        min_size: 0.3,
        ..HitboxOptions::default()
    };
    scene.generate_hitboxes(owner, &opts).unwrap();
    assert!(scene.world.contains(stale), "still live until the drain");
    assert_eq!(scene.take_pending_destroys(), vec![stale]);
}

#[test]
fn a_ragdoll_built_before_the_drain_skips_a_doomed_hitbox() {
    let mut scene = Scene::new();
    let owner = rig(&mut scene);
    scene
        .generate_hitboxes(owner, &HitboxOptions::default())
        .unwrap();
    let head = scene.find_bone(owner, "head").unwrap();
    scene.set_playing(true);
    let opts = HitboxOptions {
        min_size: 0.3,
        ..HitboxOptions::default()
    };
    scene.generate_hitboxes(owner, &opts).unwrap();
    assert_eq!(
        scene.hitbox_of(head),
        None,
        "a queued hitbox is gone to lookups"
    );
    let made = scene
        .build_ragdoll(owner, &RagdollOptions::default())
        .unwrap();
    assert!(made.iter().all(|(_, bone)| *bone != head));
    assert!(
        !scene.world.has_rigidbody(head),
        "no body on the doomed hitbox's bone"
    );
}

#[test]
fn a_ragdoll_built_after_a_limb_is_queued_skips_the_whole_limb() {
    let mut scene = Scene::new();
    let owner = rig(&mut scene);
    let fine = HitboxOptions {
        min_size: 0.01,
        ..HitboxOptions::default()
    };
    scene.generate_hitboxes(owner, &fine).unwrap();
    let arm = scene.find_bone(owner, "arm").unwrap();
    let finger = scene.find_bone(owner, "finger").unwrap();
    assert!(scene.hitbox_of(finger).is_some(), "the finger has its own");
    scene.set_playing(true);
    scene.destroy(arm); // `Scene.DestroyEntity(arm)`: queued with its subtree
    assert_eq!(scene.hitbox_of(finger), None, "doomed with its bone");
    let opts = RagdollOptions::default();
    let made = scene.build_ragdoll(owner, &opts).unwrap();
    assert_eq!(made.len(), 3, "pelvis, spine and head only");
    for limb in [arm, finger] {
        assert!(!scene.world.has_rigidbody(limb), "no body on a doomed bone");
    }
    let total: f32 = made
        .iter()
        .map(|(_, bone)| scene.world.rigidbody(*bone).unwrap().mass)
        .sum();
    assert!(
        (total - opts.mass).abs() < 1e-3,
        "the survivors share it all"
    );
}
