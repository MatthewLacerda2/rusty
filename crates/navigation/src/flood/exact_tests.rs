//! The goal-directed flood gives every goal the very `f32` the full Dijkstra flood does.

use glam::Vec3;

use crate::test_support::add_modifier;
use crate::{Flood, NavArea, NavigationGraph, ALL_AREAS};
use rusty_components::{ColliderShape, OffMeshLinkComponent};
use rusty_lighting::occlusion::bench::level::rooms;
use rusty_scene::ColliderComponent;

const MUD: u8 = 2;

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

fn lengths(nav: &NavigationGraph, flood: &Flood<'_>, goals: &[Vec3]) -> Vec<Option<u32>> {
    let bits = |l: Option<f32>| l.map(f32::to_bits);
    goals
        .iter()
        .map(|&g| bits(flood.length_to(nav, g)))
        .collect()
}

#[test]
fn every_goal_costs_what_the_full_flood_gives_it_to_the_bit() {
    let nav = level(10);
    let at = |[x, y, z]: [f32; 3]| Vec3::new(x, y, z);
    let queriers = [
        [5., 0., 5.],
        [45., 0., 55.],
        [95., 0., 15.],
        [5., 4., 5.],
        [55., 4., 45.],
    ];
    let targets = [[25.0, 0.0, 25.0], [65.0, 0.0, 85.0], [15.0, 4.0, 75.0]].map(at);
    let dry = ALL_AREAS & !(1 << MUD);
    let (mut reached, mut unreached, mut across, mut muddy) = (0, 0, 0, 0);
    let goals = targets.map(|t| donut(t, 12));
    for from in queriers.map(at) {
        for bound in [40.0, 220.0] {
            let full = |mask| {
                let flood = nav.flood(from, bound, mask).expect("on the navmesh");
                goals.each_ref().map(|g| lengths(&nav, &flood, g))
            };
            let (full, dry_full) = (full(ALL_AREAS), full(dry));
            for (k, (goals, target)) in goals.iter().zip(targets).enumerate() {
                for (mask, full) in [(ALL_AREAS, &full[k]), (dry, &dry_full[k])] {
                    let to = nav.flood_to(from, goals, bound, mask);
                    let directed = lengths(&nav, &to.expect("on the navmesh"), goals);
                    assert_eq!(
                        &directed, full,
                        "from {from} to {target}, bound {bound}, mask {mask:#x}"
                    );
                }
                let (full, dry_full) = (&full[k], &dry_full[k]);
                reached += full.iter().flatten().count();
                unreached += full.iter().filter(|l| l.is_none()).count();
                across += goals
                    .iter()
                    .zip(full)
                    .filter(|(g, l)| l.is_some() && (g.y - from.y).abs() > 2.0)
                    .count();
                muddy += full.iter().zip(dry_full).filter(|(a, b)| a != b).count();
            }
        }
    }
    let counts =
        format!("{reached} reached, {unreached} not, {across} across storeys, {muddy} muddy");
    assert!(reached > 100 && unreached > 100, "{counts}");
    assert!(across > 10 && muddy > 10, "{counts}");
}
