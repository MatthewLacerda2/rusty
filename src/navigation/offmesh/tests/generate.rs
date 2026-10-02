//! Which links the bake generates, and where.

use glam::Vec3;

use super::super::super::test_support::{add_box, add_floor};
use super::super::super::NavBounds;
use super::*;

#[test]
fn drops_leave_the_platform_edges_and_land_on_the_floor() {
    let g = baked(&platform());
    let drops = of_kind(&g, OffMeshLinkKind::Drop);
    let east: Vec<_> = drops
        .iter()
        .filter(|l| l.end.x > l.start.x && l.start.z < 7.5)
        .collect();
    assert!(!east.is_empty(), "the +x edge drops onto the floor");
    for l in &east {
        assert_eq!((l.start.y, l.end.y), (3.0, 0.0), "{l:?}");
        assert!(!l.bidirectional, "generated links are one-way");
        assert!(l.end.x - l.start.x <= 4.0, "within the edge reach: {l:?}");
    }
    let zs: Vec<f32> = east.iter().map(|l| l.start.z).collect();
    assert_eq!(zs, [1.0, 2.0, 4.0, 6.0], "one link per 2 m bucket");
}

#[test]
fn nothing_jumps_up_the_three_metre_platform() {
    let g = baked(&platform());
    let up: Vec<_> = links(&g)
        .into_iter()
        .filter(|l| l.end.y > l.start.y + 0.5)
        .collect();
    assert!(up.iter().all(|l| l.end.y - l.start.y <= 1.2), "{up:?}");
    assert!(
        up.iter().all(|l| l.end.y < 3.0),
        "3 m is over the 1.2 m jump height"
    );
}

#[test]
fn a_crate_under_the_jump_height_is_jumped_onto_and_dropped_off() {
    let mut scene = platform();
    add_box(
        &mut scene,
        Vec3::new(12.0, 0.0, 2.0),
        Vec3::new(16.0, 1.0, 6.0),
    );
    let g = baked(&scene);
    let onto = |l: &&OffMeshLink| l.end.y == 1.0 && l.start.y == 0.0;
    assert!(of_kind(&g, OffMeshLinkKind::Jump).iter().any(|l| onto(&l)));
    let off = of_kind(&g, OffMeshLinkKind::Drop);
    assert!(off.iter().any(|l| l.start.y == 1.0 && l.end.y == 0.0));
}

#[test]
fn a_gap_is_jumped_both_ways_within_the_jump_distance() {
    let mut scene = Scene::new();
    scene.nav_settings.bounds = Some(NavBounds::new(0.0, 20.0, 0.0, 6.0));
    scene.nav_settings.jump_distance = 2.0;
    add_floor(&mut scene, -1.0, 9.0, -1.0, 7.0);
    add_floor(&mut scene, 11.0, 21.0, -1.0, 7.0); // a 2 m gap over x 9..11
    let g = baked(&scene);
    let jumps = of_kind(&g, OffMeshLinkKind::Jump);
    assert!(jumps.iter().any(|l| l.end.x > 10.0 && l.start.x < 10.0));
    assert!(jumps.iter().any(|l| l.end.x < 10.0 && l.start.x > 10.0));
    scene.nav_settings.jump_distance = 0.0;
    assert!(
        links(&baked(&scene)).is_empty(),
        "no jumps when switched off"
    );
}

#[test]
fn walls_and_trimmed_floor_make_no_links() {
    let mut scene = Scene::new();
    scene.nav_settings.bounds = Some(NavBounds::new(0.0, 20.0, 0.0, 20.0));
    scene.nav_settings.jump_distance = 3.0;
    scene.nav_settings.jump_height = 1.0;
    scene.nav_settings.drop_height = 4.0;
    add_floor(&mut scene, -1.0, 21.0, -1.0, 21.0);
    // A 3 m wall ending mid-floor, and a tall pillar: the floor erodes around
    // both, but it carries on, so nothing is a gap or a ledge to jump.
    add_box(
        &mut scene,
        Vec3::new(5.0, 0.0, 0.0),
        Vec3::new(6.0, 3.0, 12.0),
    );
    add_box(
        &mut scene,
        Vec3::new(12.0, 0.0, 12.0),
        Vec3::new(13.0, 3.0, 13.0),
    );
    let g = baked(&scene);
    assert_eq!(links(&g), vec![], "only walls: no off-mesh links");
}

#[test]
fn generation_is_off_by_default() {
    let mut scene = platform();
    scene.nav_settings.drop_height = 0.0;
    scene.nav_settings.jump_height = 0.0;
    scene.nav_settings.jump_distance = 0.0;
    assert!(links(&baked(&scene)).is_empty());
}
