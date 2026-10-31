//! The opaque pass's draw order (#1499): runs of equal keys, nearest first by a
//! quantized depth, so instancing (#470) survives and near-equal batches keep their
//! key order. No GPU.

use super::sort::depth_bucket;
use super::tests::item;
use super::{DrawItem, FrameDraws};

/// `item` of mesh `mesh`, tagged `tag`, at view depth `depth`.
fn at(mesh: &str, tag: f32, depth: f32) -> DrawItem {
    DrawItem {
        depth,
        ..item(mesh, 0, 1.0, tag)
    }
}

fn meshes(draws: &FrameDraws) -> Vec<&str> {
    draws.opaque.iter().map(|b| b.key.mesh.0.as_str()).collect()
}

#[test]
fn layered_walls_draw_nearest_first_whatever_the_given_order() {
    // Given back to front, as the worst case for overdraw.
    let opaque = vec![
        at("Far", 0.0, 40.0),
        at("Mid", 1.0, 20.0),
        at("Near", 2.0, 5.0),
    ];
    let draws = FrameDraws::build(opaque, Vec::new(), true);
    assert_eq!(meshes(&draws), ["Near", "Mid", "Far"]);
}

#[test]
fn a_run_draws_at_its_nearest_instance_and_stays_one_draw() {
    // Two crates far away and one at the camera: one draw, ahead of the wall.
    let opaque = vec![
        at("Crate", 0.0, 50.0),
        at("Wall", 1.0, 10.0),
        at("Crate", 2.0, 60.0),
        at("Crate", 3.0, 2.0),
    ];
    let draws = FrameDraws::build(opaque, Vec::new(), true);
    assert_eq!(meshes(&draws), ["Crate", "Wall"]);
    assert_eq!(draws.opaque[0].instances.len(), 3);
    // Inside the run, the copies are nearest first too.
    let tags: Vec<_> = draws.instances.iter().map(|i| i.model_matrix[12]).collect();
    assert_eq!(tags, [3.0, 0.0, 2.0, 1.0]);
    // Uniform slots follow the draw order, so `pack()` still lines up.
    let slots: Vec<_> = draws.opaque.iter().map(|b| b.uniform_slot).collect();
    assert_eq!(slots, [0, 1]);
}

#[test]
fn near_equal_depths_keep_the_key_order() {
    // Within one bucket the material (key) order holds, so state changes stay few.
    assert_eq!(depth_bucket(10.0), depth_bucket(10.5));
    let opaque = vec![at("B", 0.0, 10.0), at("A", 1.0, 10.5)];
    let draws = FrameDraws::build(opaque, Vec::new(), true);
    assert_eq!(meshes(&draws), ["A", "B"]);
}

#[test]
fn buckets_grow_with_distance_and_clamp_at_the_camera() {
    assert!(depth_bucket(1.0) < depth_bucket(2.0));
    assert!(depth_bucket(100.0) < depth_bucket(200.0));
    // Beside or behind the camera, and a NaN, all count as nearest.
    let nearest = depth_bucket(0.0);
    assert_eq!(depth_bucket(-30.0), nearest);
    assert_eq!(depth_bucket(f32::NAN), nearest);
    assert!(nearest < depth_bucket(1.0));
}

#[test]
fn without_instancing_the_given_order_is_kept() {
    let opaque = vec![at("Far", 0.0, 40.0), at("Near", 1.0, 5.0)];
    let draws = FrameDraws::build(opaque, Vec::new(), false);
    assert_eq!(meshes(&draws), ["Far", "Near"]);
}
