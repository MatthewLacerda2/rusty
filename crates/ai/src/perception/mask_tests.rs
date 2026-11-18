//! Sight walks only the layers some perceiver's `sight_mask` holds, and that
//! changes nothing it sees: the prefiltered run matches the unfiltered one tick
//! for tick. With no perceiver at all, a step touches nothing.

use glam::Vec3;

use super::test_world::{perceiver, target, wall, Arena, DT, PLAYER};
use super::{perceivers, settle, sight, SightRays};
use rusty_scene::Scene;

/// The layer of characters no perceiver looks for (the stress yard's soldiers).
const HIDDEN: u8 = 5;

/// A character on `layer` at `at` with two hitboxes on [`HIDDEN`] beside it.
fn character(s: &mut Scene, at: Vec3, layer: u8) -> [u32; 3] {
    let body = target(s, at);
    s.world.set_layer(body, layer);
    let hit = [-0.4, 0.4].map(|x| {
        let h = target(s, Vec3::ZERO);
        s.set_parent(h, Some(body)).expect("parent");
        s.world.transform_mut(h).expect("hitbox").position = Vec3::new(x, 0.0, 0.0);
        s.world.set_layer(h, HIDDEN);
        h
    });
    [body, hit[0], hit[1]]
}

/// A hidden character with a perceiver on its `Head`, four visible characters
/// (one behind a wall) and a hidden soldier in plain view; returns the arena,
/// the head, every entity it must never see, and the visible bodies.
fn yard() -> (Arena, u32, Vec<u32>, Vec<u32>) {
    let (mut head, mut never, mut shown) = (0, Vec::new(), Vec::new());
    let mut a = Arena::new(|s| {
        never.extend(character(s, Vec3::ZERO, HIDDEN));
        head = perceiver(s, Vec3::ZERO);
        s.set_parent(head, Some(never[0])).expect("parent");
        s.world.transform_mut(head).expect("head").position = Vec3::Y * 0.7;
        let mut p = s.world.ai_perception_mut(head).expect("head");
        p.eye_height = 0.0; // inside its own capsule, which must not block
        p.blocking_mask = 1 | 1 << HIDDEN;
        drop(p);
        let soldier = character(s, Vec3::new(3.0, 0.0, 5.0), HIDDEN);
        // A hitbox on a seen layer is still the hidden soldier's, never a target.
        s.world.set_layer(soldier[1], PLAYER);
        never.extend(soldier);
        for x in [-3.0, -1.0, 1.0, 3.0] {
            let [body, ..] = character(s, Vec3::new(x, 0.0, 10.0), PLAYER);
            shown.push(body);
        }
        wall(s, (2.0, 4.0), (8.0, 8.5));
    });
    a.rays.per_tick = 2; // below the pair count: the round-robin carries answers
    (a, head, never, shown)
}

/// One step with the targets unfiltered: the sight list before the prefilter.
fn step_unfiltered(a: &mut Arena) {
    let ids = perceivers(&a.scene);
    let seen = sight::look_on(&a.scene, Some(&a.physics), &ids, &mut a.rays, u32::MAX);
    for (id, visible) in ids.into_iter().zip(seen) {
        let mut c = a.scene.world.ai_perception_mut(id).expect("perceiver");
        settle(&mut c, &visible, DT);
    }
}

#[test]
fn a_head_on_a_masked_out_character_sees_the_same_as_the_unfiltered_list() {
    let (mut a, head, never, shown) = yard();
    let (mut b, ..) = yard();
    for tick in 0..8 {
        a.step(1);
        step_unfiltered(&mut b);
        assert_eq!(a.memory(head), b.memory(head), "tick {tick}");
        assert_eq!(a.rays, b.rays, "tick {tick}");
    }
    let m = a.memory(head);
    assert!(never.iter().all(|t| !m.known.contains_key(t)), "{m:?}");
    assert!(shown[..3].iter().all(|&t| a.sees(head, t)), "{m:?}");
    assert!(!a.sees(head, shown[3]), "behind the wall");
}

#[test]
fn a_step_with_colliders_and_no_perceiver_leaves_the_sight_budget_as_it_was() {
    let mut bot = 0;
    let mut a = Arena::new(|s| {
        bot = perceiver(s, Vec3::ZERO);
        for x in [-2.0, 2.0] {
            character(s, Vec3::new(x, 0.0, 4.0), PLAYER);
        }
    });
    a.step(1);
    let before = a.rays.clone();
    assert_ne!(
        before,
        SightRays::default(),
        "the bot's step moved the round"
    );
    a.scene.world.set_ai_perception(bot, None);
    a.step(3);
    assert_eq!(a.rays, before);
}
