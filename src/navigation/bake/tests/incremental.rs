//! The incremental rebake (#456): a `sync` after any edit gives, bit for bit, the
//! navmesh a full bake of the edited scene gives, and touches only the cells near
//! the edit.

use super::super::super::test_support::move_to;
use super::super::super::test_support::{add_box, add_floor, add_rotated_box, assert_same_mesh};
use super::super::super::{NavBounds, NavigationGraph, Rebake};
use crate::scene::Scene;
use glam::{Quat, Vec3};

/// A 30 × 30 room on a pinned grid: walls, crates, a ramp onto a deck.
fn level(radius: f32, spacing: f32) -> (Scene, Vec<u32>) {
    let mut scene = Scene::new();
    scene.nav_settings.bounds = Some(NavBounds::new(-16.0, 16.0, -16.0, 16.0));
    scene.nav_settings.agent_radius = radius;
    scene.nav_settings.grid_spacing = spacing;
    add_floor(&mut scene, -15.0, 15.0, -15.0, 15.0);
    let mut ids = Vec::new();
    for i in 0..6 {
        let x = -12.0 + 4.5 * i as f32;
        let crate_min = Vec3::new(x, 0.0, (i as f32 * 2.3) - 6.0);
        ids.push(add_box(
            &mut scene,
            crate_min,
            crate_min + Vec3::new(1.2, 1.0, 1.2),
        ));
    }
    ids.push(add_box(
        &mut scene,
        Vec3::new(-2.0, 0.0, 4.0),
        Vec3::new(9.0, 3.0, 4.5),
    ));
    // A deck at y = 3 over z 8..12, reached by a ramp.
    add_box(
        &mut scene,
        Vec3::new(0.0, 2.8, 8.0),
        Vec3::new(8.0, 3.0, 12.0),
    );
    let ramp = Quat::from_rotation_x(-0.4);
    add_rotated_box(
        &mut scene,
        Vec3::new(-3.0, 1.5, 10.0),
        Vec3::new(2.0, 0.2, 8.0),
        ramp,
        true,
    );
    (scene, ids)
}

fn baked(scene: &Scene) -> NavigationGraph {
    let mut g = NavigationGraph::from_scene(scene);
    g.bake(scene);
    g
}

/// Every edit kind in turn; after each, `sync` must equal a fresh full bake.
fn edits_match_a_full_bake(radius: f32, spacing: f32) {
    let (mut scene, ids) = level(radius, spacing);
    let mut g = baked(&scene);
    let edits: [&dyn Fn(&mut Scene); 5] = [
        &|s| move_to(s, ids[0], Vec3::new(3.3, 0.5, -2.1)),
        &|s| {
            s.world.set_static(ids[1], false);
        },
        &|s| {
            add_box(s, Vec3::new(5.0, 0.0, 5.0), Vec3::new(5.4, 2.0, 7.0));
        },
        &|s| {
            s.world.set_active(ids[6], false);
        },
        &|s| move_to(s, ids[2], Vec3::new(12.0, 0.5, 12.0)),
    ];
    for (i, edit) in edits.iter().enumerate() {
        edit(&mut scene);
        let outcome = g.sync(&scene);
        assert!(
            matches!(outcome, Rebake::Incremental(_)),
            "edit {i}: {outcome:?}"
        );
        assert_same_mesh(&g, &baked(&scene));
    }
}

#[test]
fn incremental_rebake_equals_a_full_bake() {
    edits_match_a_full_bake(0.5, 1.0);
}

#[test]
fn incremental_rebake_equals_a_full_bake_at_fine_spacing_and_wide_radius() {
    edits_match_a_full_bake(0.9, 0.5);
    edits_match_a_full_bake(0.0, 0.5);
}

#[test]
fn an_unchanged_scene_does_not_rebake() {
    let (scene, _) = level(0.5, 1.0);
    let mut g = baked(&scene);
    let generation = g.bake_generation;
    assert_eq!(g.sync(&scene), Rebake::Unchanged);
    assert_eq!(g.bake_generation, generation);
}

#[test]
fn a_moved_crate_rebakes_only_near_its_old_and_new_place() {
    let (mut scene, ids) = level(0.5, 1.0);
    let mut g = baked(&scene);
    move_to(&mut scene, ids[0], Vec3::new(-11.4, 0.5, -3.0));
    let Rebake::Incremental(rects) = g.sync(&scene) else {
        panic!("expected an incremental rebake");
    };
    let cells: usize = rects.iter().map(|r| r.cells()).sum();
    assert!(
        cells < 60,
        "rebaked {cells} of {} cells",
        g.width * g.height
    );
}

#[test]
fn a_settings_change_rebakes_in_full() {
    let (mut scene, _) = level(0.5, 1.0);
    let mut g = baked(&scene);
    scene.nav_settings.agent_radius = 0.8;
    assert_eq!(g.sync(&scene), Rebake::Full);
    assert_same_mesh(&g, &baked(&scene));
}
