//! Tests for `NavigationGraph::reachable_spans` (#1737).

use glam::Vec3;

use crate::navigation::test_support::{add_box, add_floor, bake_pinned};
use crate::scene::Scene;

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

#[test]
fn an_actor_start_makes_its_area_reachable() {
    let mut scene = floor_and_block();
    let g = bake_pinned(&mut scene);
    let reach = g.reachable_spans(&[Vec3::new(7.5, 3.1, 7.5)]);
    let top = g.span_under(Vec3::new(7.5, 3.0, 7.5)).unwrap();
    assert!(reach[top.index as usize], "an actor starts there");
}

#[test]
fn nothing_walkable_reaches_nothing() {
    let g = crate::navigation::NavigationGraph::from_scene(&Scene::new());
    assert!(g.reachable_spans(&[Vec3::ZERO]).iter().all(|r| !r));
}
