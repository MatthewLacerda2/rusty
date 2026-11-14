//! Light probes over a baked nav surface's floor (#1737): in the air actors move
//! through, never behind a thin wall.

use glam::Vec3;

use super::site_on_nav;
use rusty_lighting::placement::test_level::block;
use rusty_lighting::placement::test_sites::{roofed_house, yard};

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
