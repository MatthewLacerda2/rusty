//! Hearing and damage (#1900): a noise is heard through walls inside
//! `hearing_range × loudness` and not past it; a hit is sensed by the struck
//! entity's perceiver at the instigator.

use glam::Vec3;

use super::test_world::{perceiver, target, wall, Arena};
use super::{forget, report_damage, report_noise};
use crate::components::Sense;

#[test]
fn a_noise_is_heard_through_a_wall_inside_its_range_only() {
    let (mut bot, mut player) = (0, 0);
    let mut a = Arena::new(|s| {
        bot = perceiver(s, Vec3::ZERO);
        player = target(s, Vec3::new(0.0, 0.0, -20.0));
        wall(s, (-5.0, 5.0), (-10.0, -9.5));
    });
    let gun = Vec3::new(0.0, 1.0, -20.0);
    assert_eq!(
        report_noise(&mut a.scene, gun, 0.5, Some(player), "gunshot"),
        0,
        "15 m"
    );
    assert!(a.updates().is_empty());
    assert_eq!(
        report_noise(&mut a.scene, gun, 1.0, Some(player), "gunshot"),
        1,
        "30 m"
    );
    let heard = a.updates();
    assert_eq!(heard.len(), 1);
    assert_eq!(heard[0].1.target, Some(player));
    assert_eq!(
        (heard[0].1.info.sense, heard[0].1.info.tag.as_str()),
        (Sense::Hearing, "gunshot")
    );
    a.step(1);
    let k = a.memory(bot).known[&player].clone();
    assert!(
        k.sensed && k.position == gun,
        "sensed through the step it was reported in"
    );
    a.step(1);
    let k = a.memory(bot).known[&player].clone();
    assert!(!k.sensed && k.age > 0.0);
}

#[test]
fn a_hit_is_sensed_at_the_instigator_by_the_struck_hierarchy() {
    let (mut bot, mut hitbox, mut player) = (0, 0, 0);
    let mut a = Arena::new(|s| {
        bot = perceiver(s, Vec3::ZERO);
        hitbox = s.add_entity("hitbox".to_string());
        s.set_parent(hitbox, Some(bot)).expect("parent");
        player = target(s, Vec3::new(0.0, 0.0, -6.0));
    });
    let at = Vec3::new(0.0, 1.0, -0.2);
    assert_eq!(
        report_damage(&mut a.scene, hitbox, Some(player), at, (25.0, "bullet")),
        Some(bot)
    );
    let k = a.memory(bot).known[&player].clone();
    assert_eq!(
        (k.sense, k.amount, k.tag.as_str()),
        (Sense::Damage, 25.0, "bullet")
    );
    assert!(
        k.position.distance(Vec3::new(0.0, 0.9, -6.0)) < 1e-4,
        "at the instigator"
    );
    // No instigator: sensed at the hit point, stored under nobody.
    a.updates();
    report_damage(&mut a.scene, bot, None, at, (5.0, "fall"));
    let u = a.updates();
    assert_eq!(
        (u.len(), u[0].1.target, u[0].1.info.position),
        (1, None, at)
    );
    forget(&mut a.scene, bot, None);
    assert!(a
        .scene
        .world
        .ai_perception(bot)
        .expect("bot")
        .memory
        .known
        .is_empty());
}
