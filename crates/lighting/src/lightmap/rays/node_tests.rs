use std::cell::RefCell;

use glam::Vec3;
use proptest::prelude::*;
use rapier3d::parry::bounding_volume::Aabb;
use rapier3d::parry::partitioning::{BvhNode, TraversalAction};
use rapier3d::parry::query::Ray;

use super::{hit, NodeRay, Rays};
use crate::lightmap::bake::triangles;
use crate::lightmap::progressive_tests::lit;
use crate::lightmap::{BakeScene, Rng};
use crate::occlusion::test_level::corridors;

fn key(t: f32) -> u32 {
    (t + 0.0).to_bits()
}

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

fn case() -> impl Strategy<Value = (Vec3, Vec3, Ray, f32)> {
    let corner = || [-50.0f32..50.0, -50.0f32..50.0, -50.0f32..50.0];
    let sizes = [0, 1, 2].map(|_| prop_oneof![Just(0.0f32), 0.0f32..20.0]);
    (corner(), sizes).prop_flat_map(|(lo, size)| {
        let mins = Vec3::from_array(lo);
        let maxs = mins + Vec3::from_array(size);
        let axis = move |i: usize| prop_oneof![Just(mins[i]), Just(maxs[i]), component()];
        let reach = prop_oneof![Just(f32::MAX), Just(0.0f32), 0.0f32..100.0, component()];
        let o = [axis(0), axis(1), axis(2)];
        let d = [component(), component(), component()];
        let ray = (o, d).prop_map(|(o, d)| Ray::new(Vec3::from_array(o), Vec3::from_array(d)));
        (ray, reach).prop_map(move |(ray, reach)| (mins, maxs, ray, reach))
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(4096))]

    #[test]
    fn the_node_test_is_parrys_to_the_bit((mins, maxs, ray, reach) in case()) {
        let node = BvhNode::leaf(Aabb::new(mins, maxs), 0);
        let ours = NodeRay::new(&ray).cast(&node, reach);
        prop_assert_eq!(key(ours), key(node.cast_ray(&ray, reach)));
    }
}

type Walk = (Vec<u32>, Option<u32>, Option<u32>);

fn walk(rays: &Rays, ray: &Ray, reach: f32, test: impl Fn(&BvhNode, f32) -> f32) -> Walk {
    let mesh = rays.mesh.as_ref().expect("a level with triangles");
    let log = RefCell::new(Vec::new());
    let logged = |node: &BvhNode, best: f32| {
        let t = test(node, best);
        log.borrow_mut().push(key(t));
        t
    };
    let bvh = mesh.bvh();
    let nearest = bvh.find_best(reach, logged, |i, best| hit(mesh, i, ray, best));
    let mut blocker = None;
    bvh.traverse(|node| {
        if logged(node, reach) == f32::MAX {
            return TraversalAction::Prune;
        }
        blocker = node
            .leaf_data()
            .filter(|&i| hit(mesh, i, ray, reach).is_some());
        match blocker {
            Some(_) => TraversalAction::EarlyExit,
            None => TraversalAction::Continue,
        }
    });
    (log.into_inner(), nearest.map(|(i, _)| i), blocker)
}

fn same_walks(rays: &Rays) {
    let axes = [Vec3::X, Vec3::Y, Vec3::Z]
        .into_iter()
        .flat_map(|a| [a, -a]);
    let (mut tested, mut found) = (0, 0);
    for (i, tri) in rays.tris.iter().enumerate() {
        let centre = tri.a + (tri.e1 + tri.e2) / 3.0;
        let origin = centre + tri.normal().normalize_or_zero() * 1e-3;
        let mut rng = Rng::new(2325, 0, i as u64);
        let mut seeded = || Vec3::from_array([0; 3].map(|_| rng.next_f32() * 2.0 - 1.0));
        let dirs = axes.clone().chain([seeded(), seeded(), seeded()]);
        for (dir, reach) in dirs.flat_map(|d| [(d, 0.5), (d, f32::MAX)]) {
            let ray = Ray::new(origin, dir);
            let boxes = NodeRay::new(&ray);
            let parry = walk(rays, &ray, reach, |node, best| node.cast_ray(&ray, best));
            let ours = walk(rays, &ray, reach, |node, best| boxes.cast(node, best));
            assert_eq!(parry, ours, "triangle {i}, {dir} reaching {reach}");
            tested += ours.0.len();
            found += ours.1.is_some() as usize;
        }
    }
    assert!(
        tested > 0 && found > 0,
        "the walks tested nodes, hit triangles"
    );
}

#[test]
fn a_lightmap_level_walks_the_same_through_either_node_test() {
    same_walks(&Rays::build(triangles(&lit())));
}

#[test]
fn the_corridors_walk_the_same_through_either_node_test() {
    let scene = BakeScene::gather(&corridors(), &|_, _| None);
    same_walks(&Rays::build(triangles(&scene)));
}
