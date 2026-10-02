//! A bridge over a walkway: a deck at y = 2.5 reached by real (tilted) ramps, with
//! the ground still walkable underneath.

use glam::Vec3;
use rusty::navigation::{NavBounds, NavigationGraph};
use rusty::scene::Scene;

use super::{add_box, add_ramp, bake, heights};

/// Ground 0..30 × 0..20; ramps over x = 2..8 and 22..28 up to a deck (top 2.5) over
/// x = 8..22, all over z = 7..13.
fn bridge() -> NavigationGraph {
    let mut scene = Scene::new();
    add_box(
        &mut scene,
        Vec3::new(0.0, -0.2, 0.0),
        Vec3::new(30.0, 0.0, 20.0),
    );
    add_box(
        &mut scene,
        Vec3::new(8.0, 2.3, 7.0),
        Vec3::new(22.0, 2.5, 13.0),
    );
    add_ramp(&mut scene, (2.0, 0.0), (8.0, 2.5), (7.0, 13.0));
    add_ramp(&mut scene, (22.0, 2.5), (28.0, 0.0), (7.0, 13.0));
    bake(&mut scene, NavBounds::new(0.0, 30.0, 0.0, 20.0))
}

#[test]
fn walkway_and_deck_are_both_walkable() {
    let g = bridge();
    let floors: Vec<f32> = g.spans_at(15, 10).iter().map(|s| s.y).collect();
    assert_eq!(floors.len(), 2, "ground and deck: {floors:?}");
    assert_eq!(floors[0], 0.0);
    assert!((floors[1] - 2.5).abs() < 1e-4, "{floors:?}");
}

#[test]
fn the_walkway_passes_under_the_bridge() {
    let g = bridge();
    let path = g
        .path_between(Vec3::new(15.0, 0.0, 2.0), Vec3::new(15.0, 0.0, 18.0))
        .expect("under the deck");
    assert!(heights(&g, &path).iter().all(|&y| y == 0.0));
}

#[test]
fn a_ramp_is_a_slope_not_a_plateau() {
    let g = bridge();
    let ramp: Vec<f32> = (3..=7)
        .map(|gx| g.spans_at(gx, 10).last().expect("ramp").y)
        .collect();
    for w in ramp.windows(2) {
        let rise = w[1] - w[0];
        assert!(
            rise > 0.3 && rise < 0.5,
            "a steady climb per cell: {ramp:?}"
        );
    }
    assert!(
        ramp[2] > 1.0 && ramp[2] < 1.6,
        "mid-ramp is mid-height: {ramp:?}"
    );
}

#[test]
fn path_onto_the_deck_climbs_the_ramp() {
    let g = bridge();
    let path = g
        .path_between(Vec3::new(1.0, 0.0, 10.0), Vec3::new(15.0, 2.5, 10.0))
        .expect("the ramp reaches the deck");
    let ys = heights(&g, &path);
    for w in ys.windows(2) {
        assert!(w[1] >= w[0] - 1e-4, "never descends on the way up: {ys:?}");
    }
    assert!(
        (ys.last().copied().unwrap_or(0.0) - 2.5).abs() < 1e-4,
        "{ys:?}"
    );
    assert_eq!(
        path,
        g.path_between(Vec3::new(1.0, 0.0, 10.0), Vec3::new(15.0, 2.5, 10.0))
            .unwrap()
    );
}
