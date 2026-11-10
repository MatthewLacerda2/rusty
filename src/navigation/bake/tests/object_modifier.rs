//! `NavMeshModifier`s (#1959): a table overridden to `NotWalkable` keeps its top
//! off the navmesh but still blocks the floor under it, `ignore_from_build` takes
//! it out of the bake, children inherit their parent's modifier unless they have
//! their own. Their edits' rebakes: `object_modifier_sync`.

use super::super::super::test_support::{add_box, add_floor};
use super::super::super::{NavBounds, NavigationGraph, NOT_WALKABLE_AREA};
use crate::components::NavMeshModifierComponent as Modifier;
use crate::scene::Scene;
use glam::Vec3;

/// A 20 × 10 floor on a pinned unit grid, no erosion, and a table at (10, 5): a
/// 3 × 3 top at 0.8 m on four legs, all children of an empty `table` root.
pub fn room() -> (Scene, u32, u32) {
    let mut scene = Scene::new();
    scene.nav_settings.grid_spacing = 1.0;
    scene.nav_settings.bounds = Some(NavBounds::new(0.0, 20.0, 0.0, 10.0));
    scene.nav_settings.agent_radius = 0.0;
    add_floor(&mut scene, -1.0, 21.0, -1.0, 11.0);
    let table = scene.add_entity("table".to_string());
    let top = add_box(
        &mut scene,
        Vec3::new(8.6, 0.7, 3.6),
        Vec3::new(11.4, 0.8, 6.4),
    );
    scene.world.set_parent_id(top, Some(table));
    for (x, z) in [(8.6, 3.6), (11.3, 3.6), (8.6, 6.3), (11.3, 6.3)] {
        let leg = add_box(
            &mut scene,
            Vec3::new(x, 0.0, z),
            Vec3::new(x + 0.1, 0.7, z + 0.1),
        );
        scene.world.set_parent_id(leg, Some(table));
    }
    (scene, table, top)
}

pub fn baked(scene: &Scene) -> NavigationGraph {
    let mut g = NavigationGraph::from_scene(scene);
    g.bake(scene);
    g
}

pub fn set(scene: &mut Scene, id: u32, m: Modifier) {
    scene.world.set_nav_mesh_modifier(id, Some(m));
}

pub fn not_walkable() -> Modifier {
    Modifier {
        override_area: Some(NOT_WALKABLE_AREA),
        ..Default::default()
    }
}

#[test]
fn without_a_modifier_the_tabletop_is_walkable() {
    let (scene, _, _) = room();
    let g = baked(&scene);
    assert_eq!(
        g.spans_at(10, 5).len(),
        1,
        "the top only: the floor lacks headroom"
    );
    assert!((g.spans_at(10, 5)[0].y - 0.8).abs() < 1e-4);
}

#[test]
fn a_not_walkable_table_blocks_the_floor_under_it_and_walks_around() {
    let (mut scene, table, _) = room();
    set(&mut scene, table, not_walkable());
    let g = baked(&scene);
    assert!(!g.is_walkable(10, 5), "no top, and no headroom under it");
    assert!(
        g.is_walkable(7, 5) && g.is_walkable(12, 5),
        "the floor around it"
    );
    assert!(g.is_walkable(10, 2) && g.is_walkable(10, 7));
}

#[test]
fn ignore_from_build_leaves_the_floor_under_it_walkable() {
    let (mut scene, table, _) = room();
    let ignored = Modifier {
        ignore_from_build: true,
        ..Default::default()
    };
    set(&mut scene, table, ignored);
    let g = baked(&scene);
    assert_eq!(g.spans_at(10, 5).len(), 1);
    assert!(
        g.spans_at(10, 5)[0].y.abs() < 1e-4,
        "the floor, as if no table"
    );
}

#[test]
fn a_child_inherits_its_parents_modifier_unless_it_has_its_own() {
    let (mut scene, table, top) = room();
    set(&mut scene, table, not_walkable());
    let area = |area| Modifier {
        override_area: Some(area),
        ..Default::default()
    };
    set(&mut scene, top, area(4));
    let g = baked(&scene);
    assert_eq!(g.spans_at(10, 5)[0].area, 4, "the top's own modifier wins");
    let no_children = Modifier {
        apply_to_children: false,
        ..area(6)
    };
    set(&mut scene, table, no_children);
    scene.world.set_nav_mesh_modifier(top, None);
    assert_eq!(
        baked(&scene).spans_at(10, 5)[0].area,
        0,
        "not applied to children"
    );
}

#[test]
fn a_low_not_walkable_seat_on_the_floor_stays_off_the_navmesh() {
    let (mut scene, _, _) = room();
    scene.nav_settings.max_step = 0.5; // the 0.45 m seat sits within it
    let sofa = add_box(
        &mut scene,
        Vec3::new(2.6, 0.0, 2.6),
        Vec3::new(4.4, 0.45, 4.4),
    );
    set(&mut scene, sofa, not_walkable());
    let g = baked(&scene);
    assert!(
        !g.is_walkable(3, 3),
        "within max_step, yet the seat is not floor"
    );
    assert!(g.is_walkable(1, 3) && g.is_walkable(5, 3) && g.is_walkable(3, 6));
}
