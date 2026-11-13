//! Light probes from the occupancy grid (#1737), and the whole site at Dust2's
//! scale. Light probes over a baked nav surface: `navigation::probe_site::light_tests`.

use glam::Vec3;

use super::test_level::block;

use super::{plan_reflection_probes, ReflectionPlacement, Site};

/// Dust2's scale (#1737): a 200 × 200 m level with a 40 m tower keeps the 0.5 m
/// cell, in about 1.6 MB, and its whole site (floor plan, occupancy grid, both
/// probe sets) plans.
#[test]
fn a_dust2_sized_site_keeps_the_fine_cell() {
    let mut scene = super::test_level::grid_of_rooms(10, 20.0);
    block(
        &mut scene,
        Vec3::new(98.0, 0.0, 98.0),
        Vec3::new(102.0, 40.0, 102.0),
    );
    let site = Site::survey(&scene, None, None, 0.5).expect("a site");
    assert_eq!(site.occupancy.cell, super::occupancy::CELL);
    let bytes = site.occupancy.memory_bytes();
    assert!(bytes <= 2_000_000, "{bytes} bytes");
    let reflections = plan_reflection_probes(&scene, &site, &ReflectionPlacement::default(), &[]);
    assert!(reflections.len() >= 100, "a probe per room at least");
    assert!(!site.plan_light_probes(4.0, &[]).is_empty());
}

/// A 2 m walkway at 3 m over open ground: the ground's lattice points that share
/// its keys sit inside its slab and are skipped, and the walkway keeps its own.
#[test]
fn a_walkway_keeps_its_probes_over_the_grounds_skipped_ones() {
    let mut scene = crate::Scene::new();
    block(
        &mut scene,
        Vec3::new(0.0, -0.2, 0.0),
        Vec3::new(12.0, 0.0, 12.0),
    );
    block(
        &mut scene,
        Vec3::new(0.0, 2.6, 7.0),
        Vec3::new(12.0, 3.0, 9.0),
    );
    let occ = super::Occupancy::from_static_boxes(&scene, super::occupancy::CELL).unwrap();
    let mut floor = super::FloorGrid::from_static(&scene, 1.0).unwrap();
    for i in 0..floor.cells.len() {
        let z = floor.centre(i % floor.width, i / floor.width).y;
        if (7.0..9.0).contains(&z) && floor.cells[i].is_some() {
            floor.upper.push((i, 3.0));
        }
    }
    let probes = super::light::plan_light_set(&floor, &occ, 4.0, &[]);
    let on_walkway = probes
        .iter()
        .filter(|p| (7.0..9.0).contains(&p.z) && p.y > 3.0);
    assert!(
        on_walkway.count() >= 3,
        "the walkway has its probes: {probes:?}"
    );
}
