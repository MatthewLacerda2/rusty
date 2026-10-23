//! In Play a stale hitbox is queued, not removed on the spot (#1298), so the
//! destroy phase can tear down any script under it first.

use super::rig::rig;
use super::*;

#[test]
fn in_play_a_stale_hitbox_is_queued_for_the_destroy_phase() {
    let mut scene = Scene::new();
    let owner = rig(&mut scene);
    scene
        .generate_hitboxes(owner, &HitboxOptions::default())
        .unwrap();
    let head = scene.find_bone(owner, "head").unwrap();
    let stale = scene.hitbox_of(head).unwrap();
    scene.playing = true;
    let opts = HitboxOptions {
        min_size: 0.3,
        ..HitboxOptions::default()
    };
    scene.generate_hitboxes(owner, &opts).unwrap();
    assert!(scene.world.contains(stale), "still live until the drain");
    assert_eq!(scene.take_pending_destroys(), vec![stale]);
}
