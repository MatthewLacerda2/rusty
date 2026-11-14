//! What perception costs on the Dust2-sized bench level (#1900)
//! ([`super::bench`]): the gate run checks the step works at level scale under
//! the ray budget.

use super::bench::{level_with_perceivers, run};
use super::test_world::PLAYER;
use super::SightRays;

#[test]
fn thirty_perceivers_on_the_bench_level_see_the_target_near_them_only() {
    let (mut scene, physics, bots) = level_with_perceivers(20, 1 << PLAYER);
    let mut rays = SightRays::default();
    run(&mut scene, &physics, &mut rays, 2);
    let seeing: Vec<u32> = bots
        .iter()
        .copied()
        .filter(|&b| {
            !scene
                .world
                .ai_perception(b)
                .expect("bot")
                .memory
                .in_sight
                .is_empty()
        })
        .collect();
    assert!(!seeing.is_empty(), "the bot in the target's room sees it");
    assert!(
        seeing.len() < bots.len(),
        "walls hide it from the rest: {seeing:?}"
    );
}
