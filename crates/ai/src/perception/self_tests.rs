//! A perceiver never senses itself (#1900): not its own body in sight (a
//! perceiver on a `Head` child), not a noise or hit from its own hierarchy, and
//! a dead instigator is no known target. Nothing is sensed outside Play, and
//! entering Play empties every memory.

use glam::Vec3;

use super::test_world::{perceiver, target, Arena};
use super::{report_damage, report_noise};

/// A body on the target layer with a perceiver on its `Head` child; returns
/// (body, head).
fn headed(s: &mut rusty_scene::Scene, at: Vec3) -> (u32, u32) {
    let body = target(s, at);
    let head = perceiver(s, Vec3::ZERO);
    s.set_parent(head, Some(body)).expect("parent");
    s.world.transform_mut(head).expect("head").position = Vec3::Y * 0.7;
    // Eyes at the head, 1.6 m up: inside the body's box, which must not block.
    s.world.ai_perception_mut(head).expect("head").eye_height = 0.0;
    (body, head)
}

#[test]
fn a_perceiver_on_a_head_sees_others_never_its_own_body() {
    let (mut body, mut head, mut other) = (0, 0, 0);
    let mut a = Arena::new(|s| {
        (body, head) = headed(s, Vec3::ZERO);
        other = target(s, Vec3::new(0.0, 0.0, 8.0));
    });
    a.step(2);
    let m = a.memory(head);
    assert!(a.sees(head, other), "the ray leaves through its own body");
    assert!(!m.known.contains_key(&body), "{m:?}");
    assert_eq!(m.known.len(), 1);
}

#[test]
fn its_own_noise_and_hits_are_not_recorded_and_the_dead_are_no_target() {
    let (mut body, mut head, mut gun, mut player) = (0, 0, 0, 0);
    let mut a = Arena::new(|s| {
        (body, head) = headed(s, Vec3::ZERO);
        gun = s.add_entity("gun".to_string());
        s.set_parent(gun, Some(head)).expect("parent");
        player = target(s, Vec3::new(0.0, 0.0, -6.0));
    });
    let at = Vec3::new(0.0, 1.0, 1.0);
    for own in [head, body, gun] {
        assert_eq!(report_noise(&mut a.scene, at, 1.0, Some(own), "shot"), 0);
        let hit = report_damage(&mut a.scene, head, Some(own), at, (5.0, "fall"));
        assert_eq!(hit, None, "{own}");
    }
    assert!(a.updates().is_empty());
    // Gone for good (an edit-mode despawn, so Play's teardown isn't asked for);
    // re-entering Play empties the memory, which is empty here already.
    a.scene.set_playing(false);
    a.scene.destroy_entity(player);
    a.scene.set_playing(true);
    assert_eq!(report_noise(&mut a.scene, at, 1.0, Some(player), "shot"), 1);
    let u = a.updates();
    assert_eq!((u.len(), u[0].1.target), (1, None), "a dead instigator");
    assert!(a.memory(head).known.is_empty());
}

#[test]
fn nothing_is_sensed_outside_play_and_play_entry_forgets() {
    let (mut bot, mut player) = (0, 0);
    let mut a = Arena::new(|s| {
        bot = perceiver(s, Vec3::ZERO);
        player = target(s, Vec3::new(0.0, 0.0, 6.0));
    });
    a.step(1);
    let at = Vec3::new(0.0, 1.0, 6.0);
    assert_eq!(report_noise(&mut a.scene, at, 1.0, None, ""), 1);
    a.scene.set_playing(false);
    assert_eq!(report_noise(&mut a.scene, at, 1.0, Some(player), ""), 0);
    assert_eq!(report_damage(&mut a.scene, bot, None, at, (1.0, "")), None);
    assert_eq!(
        a.memory(bot).updates.len(),
        2,
        "only the gain and the noise in Play"
    );
    a.scene.set_playing(true);
    assert!(a.memory(bot).is_empty(), "Play starts knowing nothing");
}
