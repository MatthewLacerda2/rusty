//! The sight sense (#1900): range, cone, walls, the lose-sight edge, and the
//! updates a gain and a loss raise.

use glam::Vec3;

use super::test_world::{perceiver, target, wall, Arena};

#[test]
fn a_target_ahead_in_range_is_seen() {
    let (mut bot, mut player) = (0, 0);
    let mut a = Arena::new(|s| {
        bot = perceiver(s, Vec3::ZERO);
        player = target(s, Vec3::new(1.0, 0.0, 10.0));
    });
    a.step(1);
    assert!(a.sees(bot, player));
    let k = a.memory(bot).known[&player].clone();
    assert!(k.sensed && k.age == 0.0, "{k:?}");
    assert!(
        k.position.distance(Vec3::new(1.0, 0.0, 10.0)) < 1e-4,
        "where it stands, not its middle: {k:?}"
    );
}

#[test]
fn a_wall_blocks_sight() {
    let (mut bot, mut player) = (0, 0);
    let mut a = Arena::new(|s| {
        bot = perceiver(s, Vec3::ZERO);
        player = target(s, Vec3::new(0.0, 0.0, 10.0));
        wall(s, (-2.0, 2.0), (5.0, 5.5));
    });
    a.step(1);
    assert!(!a.sees(bot, player));
    assert!(a.updates().is_empty(), "nothing was sensed");
}

#[test]
fn a_target_behind_or_out_of_range_is_not_seen() {
    let (mut bot, mut behind, mut far) = (0, 0, 0);
    let mut a = Arena::new(|s| {
        bot = perceiver(s, Vec3::ZERO);
        behind = target(s, Vec3::new(0.0, 0.0, -5.0));
        far = target(s, Vec3::new(0.0, 0.0, 22.0));
    });
    a.step(1);
    assert!(!a.sees(bot, behind), "outside the 60° half-cone");
    assert!(!a.sees(bot, far), "past the 20 m sight range");
}

#[test]
fn sight_is_kept_to_the_lose_range_then_lost_once() {
    let (mut bot, mut player) = (0, 0);
    let mut a = Arena::new(|s| {
        bot = perceiver(s, Vec3::ZERO);
        player = target(s, Vec3::new(0.0, 0.0, 10.0));
    });
    a.step(1);
    let gained = a.updates();
    assert_eq!(gained.len(), 1, "{gained:?}");
    assert_eq!((gained[0].0, gained[0].1.target), (bot, Some(player)));
    assert!(gained[0].1.info.sensed && !gained[0].1.forgotten);

    a.put(player, Vec3::new(0.0, 0.9, 23.0));
    a.step(3);
    assert!(a.sees(bot, player), "kept between 20 and 25 m once seen");
    assert!(a.updates().is_empty(), "no update while it stays seen");

    a.put(player, Vec3::new(0.0, 0.9, 27.0));
    a.step(3);
    assert!(!a.sees(bot, player));
    let lost = a.updates();
    assert_eq!(lost.len(), 1, "one loss: {lost:?}");
    let info = &lost[0].1.info;
    assert!(!info.sensed && !lost[0].1.forgotten);
    assert!(
        info.position.distance(Vec3::new(0.0, 0.0, 23.0)) < 1e-4,
        "last seen there"
    );
}

#[test]
fn a_lost_target_is_forgotten_after_the_sight_max_age() {
    let (mut bot, mut player) = (0, 0);
    let mut a = Arena::new(|s| {
        bot = perceiver(s, Vec3::ZERO);
        player = target(s, Vec3::new(0.0, 0.0, 10.0));
    });
    a.step(1);
    a.put(player, Vec3::new(0.0, 0.9, -10.0));
    a.step(1);
    a.updates();
    a.step(298); // 299 ticks of age since the loss: under 5 s, still known
    assert!(a.updates().is_empty());
    let known = &a.memory(bot).known;
    assert!(known.contains_key(&player));
    a.step(2);
    let gone = a.updates();
    assert_eq!(gone.len(), 1, "{gone:?}");
    assert!(gone[0].1.forgotten && gone[0].1.target == Some(player));
    let known = &a.memory(bot).known;
    assert!(known.is_empty());
}
