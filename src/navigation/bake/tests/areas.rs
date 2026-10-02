//! `NavMeshModifierVolume`s (#460): a volume assigns its area to the floor inside
//! it, `NotWalkable` removes it, overlaps resolve by Unity's rule, and every edit to
//! a volume rebakes incrementally to exactly what a full bake gives.

use super::super::super::test_support::{add_floor, add_modifier, assert_same_mesh, move_to};
use super::super::super::{NavBounds, NavigationGraph, Rebake, NOT_WALKABLE_AREA};
use crate::scene::Scene;
use glam::Vec3;

/// A 20 × 10 floor on a pinned unit grid, no erosion.
fn field() -> Scene {
    let mut scene = Scene::new();
    scene.nav_settings.bounds = Some(NavBounds::new(0.0, 20.0, 0.0, 10.0));
    scene.nav_settings.agent_radius = 0.0;
    add_floor(&mut scene, -1.0, 21.0, -1.0, 11.0);
    scene
}

fn baked(scene: &Scene) -> NavigationGraph {
    let mut g = NavigationGraph::from_scene(scene);
    g.bake(scene);
    g
}

fn area(g: &NavigationGraph, gx: i32, gz: i32) -> u8 {
    g.spans_at(gx, gz)[0].area
}

#[test]
fn a_volume_assigns_its_area_to_the_floor_inside_it() {
    let mut scene = field();
    add_modifier(
        &mut scene,
        Vec3::new(10.0, 0.0, 5.0),
        Vec3::new(3.0, 2.0, 3.0),
        4,
    );
    let g = baked(&scene);
    assert_eq!(area(&g, 10, 5), 4);
    assert_eq!(area(&g, 11, 6), 4, "the box's edge cells are inside");
    assert_eq!(area(&g, 12, 5), 0, "outside stays Walkable");
    assert_eq!(area(&g, 2, 2), 0);
}

#[test]
fn not_walkable_removes_the_floor_and_wins_overlaps() {
    let mut scene = field();
    add_modifier(
        &mut scene,
        Vec3::new(10.0, 0.0, 5.0),
        Vec3::new(5.0, 2.0, 5.0),
        7,
    );
    add_modifier(
        &mut scene,
        Vec3::new(10.0, 0.0, 5.0),
        Vec3::new(1.0, 2.0, 1.0),
        NOT_WALKABLE_AREA,
    );
    add_modifier(
        &mut scene,
        Vec3::new(12.0, 0.0, 5.0),
        Vec3::new(1.0, 2.0, 1.0),
        3,
    );
    let g = baked(&scene);
    assert!(!g.is_walkable(10, 5), "NotWalkable beats area 7");
    assert_eq!(area(&g, 12, 5), 7, "the higher id wins an overlap");
    assert_eq!(area(&g, 9, 5), 7);
}

#[test]
fn a_volume_above_the_floor_leaves_it_alone() {
    let mut scene = field();
    add_modifier(
        &mut scene,
        Vec3::new(10.0, 3.0, 5.0),
        Vec3::new(4.0, 2.0, 4.0),
        5,
    );
    assert_eq!(area(&baked(&scene), 10, 5), 0);
}

#[test]
fn volume_edits_rebake_incrementally_to_a_full_bake() {
    let mut scene = field();
    let mut g = baked(&scene);
    let id = add_modifier(
        &mut scene,
        Vec3::new(5.0, 0.0, 5.0),
        Vec3::new(3.0, 2.0, 3.0),
        2,
    );
    let edits: [&dyn Fn(&mut Scene); 4] = [
        &|_| {},
        &|s| move_to(s, id, Vec3::new(14.0, 0.0, 4.0)),
        &|s| s.world.nav_modifier_mut(id).unwrap().area = NOT_WALKABLE_AREA,
        &|s| s.world.nav_modifier_mut(id).unwrap().active = false,
    ];
    for (i, edit) in edits.iter().enumerate() {
        edit(&mut scene);
        let outcome = g.sync(&scene);
        let Rebake::Incremental(rects) = outcome else {
            panic!("edit {i}: expected an incremental rebake, got {outcome:?}");
        };
        assert!(rects.iter().all(|r| r.cells() < 100), "edit {i}: {rects:?}");
        assert_same_mesh(&g, &baked(&scene));
    }
    assert_eq!(
        area(&g, 5, 5),
        0,
        "the deactivated volume left Walkable behind"
    );
}
