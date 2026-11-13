//! Tests for `NavigationGraph::reachable_spans` (#1737).

use glam::Vec3;

use crate::test_support::{add_box, add_floor, bake_pinned};
use rusty_scene::Scene;

/// A floor with a tall block on it: the block's top is walkable but unreachable.
fn floor_and_block() -> Scene {
    let mut scene = Scene::new();
    add_floor(&mut scene, 0.0, 10.0, 0.0, 10.0);
    add_box(
        &mut scene,
        Vec3::new(6.0, 0.0, 6.0),
        Vec3::new(9.0, 3.0, 9.0),
    );
    scene
}

#[test]
fn a_block_top_is_not_reachable_from_the_floor() {
    let mut scene = floor_and_block();
    let g = bake_pinned(&mut scene);
    let reach = g.reachable_spans(&[]);
    let top = g
        .span_under(Vec3::new(7.5, 3.0, 7.5))
        .expect("the top is walkable");
    let floor = g.span_under(Vec3::new(2.0, 0.0, 2.0)).expect("the floor");
    assert!(reach[floor.index as usize], "the largest area is reachable");
    assert!(!reach[top.index as usize], "the block top is an island");
}

/// A sentry on the block is the only actor: its top is reached, and so is the
/// main floor, which a script may spawn the player on.
#[test]
fn an_actor_start_adds_its_area_to_the_main_floor() {
    let mut scene = floor_and_block();
    let g = bake_pinned(&mut scene);
    let reach = g.reachable_spans(&[Vec3::new(7.5, 3.1, 7.5)]);
    let top = g.span_under(Vec3::new(7.5, 3.0, 7.5)).unwrap();
    let floor = g.span_under(Vec3::new(2.0, 0.0, 2.0)).unwrap();
    assert!(reach[top.index as usize], "an actor starts there");
    assert!(
        reach[floor.index as usize],
        "the main floor is always seeded"
    );
}

/// A floor over a 2 × 2 m sealed nook is not lifted by it: the floor, not the
/// block top, is the main floor.
#[test]
fn a_small_pocket_below_does_not_make_the_floor_a_top() {
    let mut scene = floor_and_block();
    add_box(
        &mut scene,
        Vec3::new(1.0, -3.1, 1.0),
        Vec3::new(3.0, -3.0, 3.0),
    );
    let g = bake_pinned(&mut scene);
    let reach = g.reachable_spans(&[]);
    let nook = g.span_under(Vec3::new(2.0, -3.0, 2.0)).expect("the nook");
    let floor = g.span_under(Vec3::new(5.0, 0.0, 2.0)).unwrap();
    let top = g.span_under(Vec3::new(7.5, 3.0, 7.5)).unwrap();
    assert!(reach[floor.index as usize], "the floor is the main floor");
    assert!(!reach[top.index as usize], "the block top is not");
    assert!(!reach[nook.index as usize], "the sealed nook is not");
}

#[test]
fn nothing_walkable_reaches_nothing() {
    let g = crate::NavigationGraph::from_scene(&Scene::new());
    assert!(g.reachable_spans(&[Vec3::ZERO]).iter().all(|r| !r));
}
