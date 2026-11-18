//! Sight walks only the layers some perceiver's `sight_mask` holds, and that
//! changes nothing it sees: the engine's run matches the rules before the
//! prefilter ([`Arena::step_reference`]) tick for tick.

use glam::Vec3;

use super::test_world::{perceiver, target, wall, Arena, PLAYER};
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

/// Run `a` (the engine) and `b` (the reference) side by side, `drive` moving
/// both before each tick; their memories and budgets match tick for tick.
fn same_as_reference(a: &mut Arena, b: &mut Arena, head: u32, drive: impl Fn(&mut Arena, usize)) {
    for tick in 0..8 {
        drive(a, tick);
        drive(b, tick);
        a.step(1);
        b.step_reference();
        assert_eq!(a.memory(head), b.memory(head), "tick {tick}");
        assert_eq!(a.rays, b.rays, "tick {tick}");
    }
}

#[test]
fn a_head_on_a_masked_out_character_sees_what_the_reference_rules_see() {
    let (mut a, head, never, shown) = yard();
    let (mut b, ..) = yard();
    same_as_reference(&mut a, &mut b, head, |_, _| {});
    let m = a.memory(head);
    assert!(never.iter().all(|t| !m.known.contains_key(t)), "{m:?}");
    assert!(shown[..3].iter().all(|&t| a.sees(head, t)), "{m:?}");
    assert!(!a.sees(head, shown[3]), "behind the wall");
}

#[test]
fn a_head_on_a_moving_seen_character_with_the_default_mask_matches_the_reference() {
    let mut ids = (0, 0);
    let build = |ids: &mut (u32, u32)| {
        Arena::new(|s| {
            let [body, ..] = character(s, Vec3::ZERO, PLAYER);
            let head = perceiver(s, Vec3::ZERO);
            s.set_parent(head, Some(body)).expect("parent");
            let mut p = s.world.ai_perception_mut(head).expect("head");
            (p.sight_mask, p.eye_height) = (u32::MAX, 0.0); // the component default
            drop(p);
            for x in [-4.0, 0.0, 4.0] {
                character(s, Vec3::new(x, 0.0, 9.0), PLAYER);
            }
            wall(s, (-1.0, 1.0), (4.0, 4.5));
            *ids = (body, head);
        })
    };
    let (mut a, mut b) = (build(&mut ids), build(&mut (0, 0)));
    a.rays.per_tick = 2;
    b.rays.per_tick = 2;
    let (body, head) = ids;
    same_as_reference(&mut a, &mut b, head, |r, tick| {
        r.put(body, Vec3::new(tick as f32 * 0.5 - 2.0, 0.9, 0.0));
    });
    let m = a.memory(head);
    assert!(!m.known.contains_key(&body), "its own capsule: {m:?}");
    assert!(!m.known.is_empty(), "the others are seen: {m:?}");
}
