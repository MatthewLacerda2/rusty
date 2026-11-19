//! The bake's BVH node test (`rays::NodeRay`, #2325) is parry's
//! `BvhNode::cast_ray` with the reciprocal hoisted: the two agree to the bit on
//! any box and ray, and a bake through either writes the same bytes after the
//! same number of node tests. A parry upgrade that changes its box test fails
//! here instead of drifting the bake.

use std::sync::atomic::Ordering::Relaxed;

use glam::Vec3;
use proptest::prelude::*;
use rapier3d::parry::bounding_volume::Aabb;
use rapier3d::parry::partitioning::BvhNode;
use rapier3d::parry::query::Ray;

use super::progressive_tests::{bits, lit};
use super::rays::{NodeRay, NODES, PARRY_NODES};
use super::tests::settings;
use crate::occlusion::bake_occlusion;
use crate::occlusion::test_level::corridors;
use rusty_scene::lighting::occlusion::encode;

/// A float from the cases that break a box test: ordinary, exact zeros, tiny
/// and huge magnitudes, the infinities and NaN, and any bit pattern at all.
fn component() -> impl Strategy<Value = f32> {
    prop_oneof![
        4 => -50.0f32..50.0,
        2 => Just(0.0f32),
        1 => Just(-0.0f32),
        1 => (1e-38f32..1e-30).prop_map(|t| t * if t.to_bits() & 1 == 0 { 1.0 } else { -1.0 }),
        1 => Just(1e30f32),
        1 => Just(f32::INFINITY),
        1 => Just(f32::NEG_INFINITY),
        1 => Just(f32::NAN),
        1 => any::<f32>(),
    ]
}

/// A box (possibly flat on any axis) and a ray whose origin, per axis, lies on
/// its near face, its far face or anywhere, plus how far the ray reaches.
fn case() -> impl Strategy<Value = (Vec3, Vec3, Ray, f32)> {
    let corner = || [-50.0f32..50.0, -50.0f32..50.0, -50.0f32..50.0];
    let sizes = [0, 1, 2].map(|_| prop_oneof![Just(0.0f32), 0.0f32..20.0]);
    (corner(), sizes).prop_flat_map(|(lo, size)| {
        let mins = Vec3::from_array(lo);
        let maxs = mins + Vec3::from_array(size);
        let axis = move |i: usize| prop_oneof![Just(mins[i]), Just(maxs[i]), component()];
        let reach = prop_oneof![Just(f32::MAX), Just(0.0f32), 0.0f32..100.0, component()];
        (
            [axis(0), axis(1), axis(2)],
            [component(), component(), component()],
            reach,
        )
            .prop_map(move |(o, d, reach)| {
                let ray = Ray::new(Vec3::from_array(o), Vec3::from_array(d));
                (mins, maxs, ray, reach)
            })
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(4096))]

    #[test]
    fn the_node_test_is_parrys_to_the_bit((mins, maxs, ray, reach) in case()) {
        let node = BvhNode::leaf(Aabb::new(mins, maxs), 0);
        let ours = NodeRay::new(&ray).cast(&node, reach);
        prop_assert_eq!(ours.to_bits(), node.cast_ray(&ray, reach).to_bits());
    }
}

/// `bake` run once through parry's node test and once through ours: the bytes
/// each wrote and how many nodes each tested.
fn both<T: PartialEq + std::fmt::Debug>(bake: impl Fn() -> T) -> [(T, u64); 2] {
    [true, false].map(|parry| {
        PARRY_NODES.store(parry, Relaxed);
        NODES.store(0, Relaxed);
        let out = bake();
        (out, NODES.load(Relaxed))
    })
}

/// The lit floor and wall: the sun's shadow rays run exactly along +X (two zero
/// direction components), every ray starts inside the root's box, and the floor
/// and wall leaves are flat boxes.
#[test]
fn a_lightmap_bakes_the_same_bytes_through_either_node_test() {
    let [parry, ours] = both(|| bits(&super::bake(&lit(), &settings())));
    assert!(parry.1 > 0, "the bake tested nodes");
    assert_eq!(parry, ours, "the same bits after the same node tests");
}

/// The corridors level: axis-aligned walls and props, so flat leaf boxes and
/// rays starting inside, on the occlusion bake's sight and shaft queries.
#[test]
fn an_occlusion_bake_writes_the_same_bytes_through_either_node_test() {
    let scene = corridors();
    let [parry, ours] = both(|| encode(&bake_occlusion(&scene, 2).0.expect("occluders")));
    assert!(parry.1 > 0, "the bake tested nodes");
    assert_eq!(parry, ours, "the same bytes after the same node tests");
}
