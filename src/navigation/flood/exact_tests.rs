//! The goal-directed flood (#2088) gives every goal the very `f32` the full
//! Dijkstra flood does: reachable, unreachable (upstairs from the ground floor,
//! which no link climbs) and past the bound alike, from several queriers to
//! several targets on the Dust2-sized bench level. The level carries what the
//! heuristic's argument rests on: off-mesh links (drops through the ceiling
//! holes and off the crates, jumps, an authored ladder whose ends share a grid
//! column and an authored shortcut costing less than the distance it spans), an
//! area costlier than floor, and a mask.

use glam::Vec3;

use crate::components::{ColliderShape, OffMeshLinkComponent};
use crate::navigation::test_support::add_modifier;
use crate::navigation::{NavArea, NavigationGraph, ALL_AREAS};
use crate::scene::lighting::occlusion::bench::level::rooms;
use crate::scene::ColliderComponent;

/// The area of the mud strip across the ground floor.
const MUD: u8 = 2;

/// The bench level's `n × n` rooms on two storeys, a box collider on every
/// block, baked at 0.25 m with drops and jumps, a strip of mud (cost 3)
/// across the ground floor at x 32..38, and the two authored links: at
/// `n = 10`, 100 × 100 m, about Dust2's size.
fn level(n: usize) -> NavigationGraph {
    let mut scene = rooms(n);
    for id in scene.world.ids_with_mesh() {
        let mut col = ColliderComponent::from_mesh_bounds(false, Vec3::ZERO, Vec3::ZERO);
        col.shape = ColliderShape::Box { size: Vec3::ONE };
        scene.world.set_collider(id, Some(col));
    }
    let settings = &mut scene.nav_settings;
    settings.grid_spacing = 0.25;
    settings.drop_height = 4.5;
    settings.jump_distance = 2.0;
    settings.areas.push(NavArea {
        name: "Mud".into(),
        cost: 3.0,
    });
    let extent = n as f32 * 10.0;
    let strip = Vec3::new(6.0, 2.0, extent);
    add_modifier(&mut scene, Vec3::new(35.0, 0.0, extent / 2.0), strip, MUD);
    let ladder = (Vec3::new(55.0, 0.0, 55.0), Vec3::new(55.0, 4.0, 55.0), true);
    let shortcut = (
        Vec3::new(15.0, 0.0, 15.0),
        Vec3::new(85.0, 0.0, 85.0),
        false,
    );
    for (start, end, bidirectional) in [ladder, shortcut] {
        let id = scene.add_entity("link".into());
        let link = OffMeshLinkComponent {
            start,
            end,
            bidirectional,
            cost_override: 1.0,
            ..Default::default()
        };
        scene.world.set_offmesh_link(id, Some(link));
    }
    let mut nav = NavigationGraph::new(0.0, 0.0, 0.0, 0.0, 0.25);
    nav.bake(&scene);
    assert!(nav.offmesh_links().count() > 0, "the bake generates links");
    let climb = nav.flood(ladder.0, 2.0, ALL_AREAS).expect("on the navmesh");
    assert_eq!(
        climb.length_to(&nav, ladder.1),
        Some(1.0),
        "the ladder is climbed"
    );
    drop(climb);
    nav
}

/// A donut of `per_ring × 2` spots round `c`, on its storey, plus one on the
/// storey above and one below.
fn donut(c: Vec3, per_ring: usize) -> Vec<Vec3> {
    let mut points: Vec<Vec3> = (0..per_ring * 2)
        .map(|k| {
            let r = if k % 2 == 0 { 4.0 } else { 10.0 };
            let a = k as f32 * std::f32::consts::TAU / per_ring as f32;
            c + r * Vec3::new(a.cos(), 0.0, a.sin())
        })
        .collect();
    points.extend([c + Vec3::Y * 4.0, c - Vec3::Y * 4.0]);
    points
}

type Lengths = Vec<Option<u32>>;

/// Each goal's length from the goal-directed flood and from the full one.
fn both(nav: &NavigationGraph, from: Vec3, goals: &[Vec3], bound: f32, mask: u32) -> [Lengths; 2] {
    let bits = |l: Option<f32>| l.map(f32::to_bits);
    let to = nav
        .flood_to(from, goals, bound, mask)
        .expect("on the navmesh");
    let directed = goals.iter().map(|&g| bits(to.length_to(nav, g))).collect();
    drop(to);
    let full = nav.flood(from, bound, mask).expect("on the navmesh");
    let full = goals
        .iter()
        .map(|&g| bits(full.length_to(nav, g)))
        .collect();
    [directed, full]
}

#[test]
fn every_goal_costs_what_the_full_flood_gives_it_to_the_bit() {
    let nav = level(10);
    let at = |x: f32, y: f32, z: f32| Vec3::new(x, y, z);
    let queriers = [
        at(5.0, 0.0, 5.0),
        at(45.0, 0.0, 55.0),
        at(95.0, 0.0, 15.0),
        at(5.0, 4.0, 5.0),
        at(55.0, 4.0, 45.0),
    ];
    let targets = [
        at(25.0, 0.0, 25.0),
        at(65.0, 0.0, 85.0),
        at(15.0, 4.0, 75.0),
    ];
    let dry = ALL_AREAS & !(1 << MUD);
    let (mut reached, mut unreached, mut across, mut muddy) = (0, 0, 0, 0);
    for &from in &queriers {
        for &target in &targets {
            let goals = donut(target, 12);
            for bound in [40.0, 220.0] {
                let [directed, full] = both(&nav, from, &goals, bound, ALL_AREAS);
                assert_eq!(directed, full, "from {from} to {target}, bound {bound}");
                let [dry_directed, dry_full] = both(&nav, from, &goals, bound, dry);
                assert_eq!(dry_directed, dry_full, "dry, from {from} to {target}");
                reached += full.iter().flatten().count();
                unreached += full.iter().filter(|l| l.is_none()).count();
                // Reached on another storey: only a link changes storeys.
                across += goals
                    .iter()
                    .zip(&full)
                    .filter(|(g, l)| l.is_some() && (g.y - from.y).abs() > 2.0)
                    .count();
                // Dearer or cut off once the mud is masked: the mud was on the way.
                muddy += full.iter().zip(&dry_full).filter(|(a, b)| a != b).count();
            }
        }
    }
    let counts =
        format!("{reached} reached, {unreached} not, {across} across storeys, {muddy} muddy");
    assert!(reached > 100 && unreached > 100, "{counts}");
    assert!(across > 10 && muddy > 10, "{counts}");
}
