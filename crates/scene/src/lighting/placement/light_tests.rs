//! Light probes from the occupancy grid (#1737), and the whole site at Dust2's
//! scale.

use glam::Vec3;

use super::test_level::block;
use super::test_sites::{roofed_house, site_on_nav, yard};
use super::{plan_reflection_probes, ReflectionPlacement, Site};

#[test]
fn light_probes_stay_in_the_air_actors_move_through() {
    let mut scene = yard();
    roofed_house(&mut scene);
    let site = site_on_nav(&scene);
    let probes = site.plan_light_probes(4.0, &[]);
    assert!(!probes.is_empty());
    for p in &probes {
        assert!(!site.occupancy.is_solid(*p), "inside geometry: {p}");
        assert!(p.y < 4.0, "above the roof: {p}");
    }
    let near_wall = probes
        .iter()
        .filter(|p| p.x.abs() < 9.0 && p.z.abs() < 9.0)
        .count();
    let open = probes
        .iter()
        .filter(|p| p.x > 9.0 && p.z.abs() < 9.0)
        .count();
    // The house's air (its room and the yard ring round it, 228 m²) against open
    // yard (108 m²): denser per square metre near the walls.
    let (near, far) = (near_wall as f32 / 228.0, open as f32 / 108.0);
    assert!(
        near > far * 1.5,
        "denser near geometry: {near_wall} vs {open}"
    );
}

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

/// A closed room of 0.2 m walls in the yard: the yard's lattice reaches past its
/// cells, but never through a wall into the room (#1737).
#[test]
fn no_light_probe_lands_behind_a_thin_wall() {
    let mut scene = yard();
    let (o, i, h) = (5.0, 4.8, 3.0);
    block(&mut scene, Vec3::new(-o, 0.0, i), Vec3::new(o, h, o));
    block(&mut scene, Vec3::new(-o, 0.0, -o), Vec3::new(o, h, -i));
    block(&mut scene, Vec3::new(-o, 0.0, -i), Vec3::new(-i, h, i));
    block(&mut scene, Vec3::new(i, 0.0, -i), Vec3::new(o, h, i));
    block(&mut scene, Vec3::new(-o, h, -o), Vec3::new(o, h + 0.2, o));
    let probes = site_on_nav(&scene).plan_light_probes(4.0, &[]);
    assert!(!probes.is_empty());
    let inside = |p: &&Vec3| p.x.abs() < i && p.z.abs() < i && p.y < h;
    assert_eq!(
        probes.iter().find(inside),
        None,
        "a probe in the closed room"
    );
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
