//! #747 — the bot chases a Player into the pool and back out, over the ramps: down
//! the north ramp from its spawn, across the bottom, and up the south ramp to the
//! Player's spawn, never dropping in over the pool's walls.

use std::collections::BTreeSet;

use glam::Vec3;
use rusty::navigation::NavigationGraph;
use rusty::scene::default_scene::layout::{DECK_TOP, POOL_FLOOR};

use super::steering::{assert_stopped, chase_with};
use super::{
    assert_in_pool_only_by_ramp, default_scene, home, planar, pool_middle, ramp_under, reach,
    BASE_OFFSET,
};

/// Every frame: below the deck only by a ramp, and note which ramp it is on.
fn watch(ramps: &mut BTreeSet<i8>, pos: Vec3, when: &str) {
    assert_in_pool_only_by_ramp(pos, when);
    ramps.extend(ramp_under(pos));
}

#[test]
fn bot_chases_into_and_out_of_the_pool_over_the_ramps() {
    let (mut scene, enemy) = default_scene();
    let graph = NavigationGraph::from_scene(&scene);
    let reach = reach(&scene, enemy);
    let mut ramps = BTreeSet::new();

    let down = chase_with(&mut scene, &graph, (enemy, pool_middle()), 900, |p, w| {
        watch(&mut ramps, p, w)
    });
    assert!(
        planar(down, pool_middle()) < reach + 0.1,
        "never reached the bottom: {down}"
    );
    assert!(
        (down.y - BASE_OFFSET - POOL_FLOOR).abs() < 0.02,
        "not on the floor: {down}"
    );
    assert_stopped(&scene, enemy, down, "on the pool floor");
    assert_eq!(ramps, BTreeSet::from([1]), "went in by the north ramp");

    let up = chase_with(&mut scene, &graph, (enemy, home()), 900, |p, w| {
        watch(&mut ramps, p, w)
    });
    assert!(planar(up, home()) < reach + 0.1, "never climbed out: {up}");
    assert!(
        (up.y - BASE_OFFSET - DECK_TOP).abs() < 0.02,
        "not on the deck: {up}"
    );
    assert_stopped(&scene, enemy, up, "back on the deck");
    assert_eq!(ramps, BTreeSet::from([-1, 1]), "came out by the south ramp");
}
