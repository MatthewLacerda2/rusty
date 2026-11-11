//! The goal-directed flood (#2088) gives every goal the very `f32` the full
//! Dijkstra flood does: reachable, unreachable (the upper storey no stair
//! reaches) and past the bound alike, from several queriers to several targets
//! on the Dust2-sized bench level.

use glam::Vec3;

use crate::components::ColliderShape;
use crate::navigation::{NavigationGraph, ALL_AREAS};
use crate::scene::lighting::occlusion::bench::level::rooms;
use crate::scene::ColliderComponent;

/// The bench level's `n × n` rooms on two storeys, a box collider on every
/// block, baked at 0.25 m: at `n = 10`, 100 × 100 m, about Dust2's size.
fn level(n: usize) -> NavigationGraph {
    let mut scene = rooms(n);
    for id in scene.world.ids_with_mesh() {
        let mut col = ColliderComponent::from_mesh_bounds(false, Vec3::ZERO, Vec3::ZERO);
        col.shape = ColliderShape::Box { size: Vec3::ONE };
        scene.world.set_collider(id, Some(col));
    }
    scene.nav_settings.grid_spacing = 0.25;
    let mut nav = NavigationGraph::new(0.0, 0.0, 0.0, 0.0, 0.25);
    nav.bake(&scene);
    nav
}

/// A donut of `per_ring × 2` spots round `c`, on the ground, plus one upstairs.
fn donut(c: Vec3, per_ring: usize) -> Vec<Vec3> {
    let mut points: Vec<Vec3> = (0..per_ring * 2)
        .map(|k| {
            let r = if k % 2 == 0 { 4.0 } else { 10.0 };
            let a = k as f32 * std::f32::consts::TAU / per_ring as f32;
            c + r * Vec3::new(a.cos(), 0.0, a.sin())
        })
        .collect();
    points.push(c + Vec3::Y * 4.0);
    points
}

/// Each goal's length from the goal-directed flood and from the full one.
fn both(nav: &NavigationGraph, from: Vec3, goals: &[Vec3], bound: f32) -> [Vec<Option<u32>>; 2] {
    let bits = |l: Option<f32>| l.map(f32::to_bits);
    let to = nav
        .flood_to(from, goals, bound, ALL_AREAS)
        .expect("on the navmesh");
    let directed = goals.iter().map(|&g| bits(to.length_to(nav, g))).collect();
    drop(to);
    let full = nav.flood(from, bound, ALL_AREAS).expect("on the navmesh");
    [
        directed,
        goals
            .iter()
            .map(|&g| bits(full.length_to(nav, g)))
            .collect(),
    ]
}

#[test]
fn every_goal_costs_what_the_full_flood_gives_it_to_the_bit() {
    let nav = level(10);
    let spot = |x: f32, z: f32| Vec3::new(x, 0.0, z);
    let queriers = [spot(5.0, 5.0), spot(45.0, 55.0), spot(95.0, 15.0)];
    let targets = [spot(25.0, 25.0), spot(65.0, 85.0), spot(15.0, 75.0)];
    let (mut reached, mut unreached) = (0, 0);
    for &from in &queriers {
        for &target in &targets {
            let goals = donut(target, 12);
            for bound in [40.0, 220.0] {
                let [directed, full] = both(&nav, from, &goals, bound);
                assert_eq!(directed, full, "from {from} to {target}, bound {bound}");
                reached += full.iter().flatten().count();
                unreached += full.iter().filter(|l| l.is_none()).count();
            }
        }
    }
    assert!(
        reached > 100 && unreached > 100,
        "{reached} reached, {unreached} not"
    );
}
