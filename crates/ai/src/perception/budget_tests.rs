//! The sight-ray budget (#1900): more pairs than rays are all served within
//! `ceil(pairs / budget)` ticks, round-robin in id order, the same way each run.

use glam::Vec3;

use super::test_world::{perceiver, target, Arena};

/// Ten perceivers in a row, all facing one target.
fn row() -> (Arena, Vec<u32>, u32) {
    let mut bots = Vec::new();
    let mut player = 0;
    let a = Arena::new(|s| {
        for i in 0..10 {
            bots.push(perceiver(s, Vec3::new(i as f32 - 5.0, 0.0, 0.0)));
        }
        player = target(s, Vec3::new(0.0, 0.0, 8.0));
    });
    (a, bots, player)
}

#[test]
fn every_perceiver_is_traced_within_ceil_n_over_budget_ticks_in_id_order() {
    let (mut a, bots, player) = row();
    a.rays.per_tick = 3;
    let seen = |a: &Arena| bots.iter().filter(|&&b| a.sees(b, player)).count();
    a.step(1);
    assert!(
        bots[..3].iter().all(|&b| a.sees(b, player)),
        "the first three"
    );
    assert_eq!(seen(&a), 3);
    a.step(1);
    assert_eq!(seen(&a), 6, "the next three, the first keep their answer");
    a.step(2); // ceil(10 / 3) = 4 ticks in all
    assert_eq!(seen(&a), 10);
}

#[test]
fn two_runs_know_the_same_things_every_tick() {
    let run = || {
        let (mut a, bots, player) = row();
        a.rays.per_tick = 4;
        let mut log = Vec::new();
        for tick in 0..120 {
            if tick == 40 {
                a.put(player, Vec3::new(0.0, 0.9, -8.0));
            }
            a.step(1);
            let memories: Vec<_> = bots.iter().map(|&b| a.memory(b)).collect();
            log.push((memories, a.updates()));
        }
        log
    };
    assert_eq!(run(), run());
}
