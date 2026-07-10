//! Tests for the height-field bake (`bake.rs`), kept in this `_tests.rs` sibling so the
//! source file stays under the size cap. Covers the stair/wall reachability rules, the
//! deterministic max-fold, and the per-scene bake settings (#276): step/slope sourced
//! from the scene, grid-spacing re-shaping, and determinism under custom settings.

use super::*;
use crate::scene::{ColliderComponent, ColliderShape, Scene};
use glam::Vec3;

/// Adds a static box collider spanning the given world AABB to `scene`.
fn add_box(scene: &mut Scene, name: &str, min: Vec3, max: Vec3) {
    let id = scene.add_entity(name.to_string());
    scene.world.set_static(id, true);
    scene.world.set_collider(
        id,
        Some(ColliderComponent {
            active: true,
            shape: ColliderShape::Box { size: max - min },
            is_trigger: false,
            aabb_min: min,
            aabb_max: max,
        }),
    );
}

/// A staircase rising 0.5/step at z = 4 (cells x = 3..=6), ground elsewhere.
fn stair_graph() -> NavigationGraph {
    let mut scene = Scene::new();
    for (i, gx) in (3..=6).enumerate() {
        let y = 0.5 * i as f32;
        let fx = gx as f32;
        add_box(
            &mut scene,
            &format!("step{i}"),
            Vec3::new(fx - 0.25, 0.0, 2.0),
            Vec3::new(fx + 0.25, y, 6.0),
        );
    }
    let mut graph = NavigationGraph::new(0.0, 10.0, 0.0, 10.0, 1.0);
    graph.bake(&scene);
    graph
}

#[test]
fn stairs_bake_rising_height_field() {
    let graph = stair_graph();
    assert_eq!(graph.height_at(3, 4), 0.0);
    assert_eq!(graph.height_at(4, 4), 0.5);
    assert_eq!(graph.height_at(5, 4), 1.0);
    assert_eq!(graph.height_at(6, 4), 1.5);
    // Each 0.5 step is within max_step, so adjacent run cells stay connected.
    for gx in 3..=5 {
        assert!(
            graph.step_connected(gx, 4, gx + 1, 4),
            "step {gx} -> {} connected",
            gx + 1
        );
    }
    // Every step cell is reachable from a neighbour within max_step.
    for gx in 3..=6 {
        assert!(graph.cell_reachable(gx, 4), "step {gx} reachable");
    }
}

#[test]
fn steep_wall_top_is_unreachable() {
    // Tall isolated box (top y=3) surrounded by ground (y=0): every neighbour is
    // a 3.0 drop, far beyond max_step 0.5, so the top is unreachable and no edge
    // connects to it — while the surrounding ground stays walkable.
    let mut scene = Scene::new();
    // Box footprint [3.6,4.4]x[3.6,4.4] covers only the single cell-center (4,4).
    add_box(
        &mut scene,
        "wall",
        Vec3::new(3.6, 0.0, 3.6),
        Vec3::new(4.4, 3.0, 4.4),
    );
    let mut graph = NavigationGraph::new(0.0, 10.0, 0.0, 10.0, 1.0);
    graph.bake(&scene);
    assert_eq!(graph.height_at(4, 4), 3.0);
    assert_eq!(graph.height_at(5, 4), 0.0, "neighbour stays ground");
    assert!(
        !graph.cell_reachable(4, 4),
        "wall top is isolated by 3.0 drops"
    );
    assert!(!graph.step_connected(3, 4, 4, 4), "cannot step up the wall");
    assert!(
        graph.cell_reachable(1, 1),
        "distant flat ground stays reachable"
    );
}

#[test]
fn bake_is_deterministic_across_runs() {
    let a = stair_graph();
    let b = stair_graph();
    assert_eq!(a.heightfield, b.heightfield, "height field must be stable");
    assert_eq!(a.walkability, b.walkability, "walkability must be stable");
    assert_eq!(a.bake_generation, b.bake_generation);
}

/// Kill "replace > with <" in raise_surface max-fold: overlapping boxes must
/// leave the cell at the MAXIMUM top, not the minimum or the first seen.
#[test]
fn raise_surface_takes_max_of_overlapping_colliders() {
    let mut scene = Scene::new();
    add_box(
        &mut scene,
        "lo",
        Vec3::new(3.0, 0.0, 3.0),
        Vec3::new(5.0, 1.0, 5.0),
    );
    add_box(
        &mut scene,
        "hi",
        Vec3::new(3.0, 0.0, 3.0),
        Vec3::new(5.0, 3.0, 5.0),
    );
    let mut g = NavigationGraph::new(0.0, 10.0, 0.0, 10.0, 1.0);
    g.bake(&scene);
    assert_eq!(g.height_at(4, 4), 3.0, "max(1.0, 3.0) = 3.0");
    assert_eq!(g.height_at(3, 3), 3.0, "corner cell also takes max");
}

/// Kill cell_reachable `<= → <`: a cell exactly max_step above a neighbour
/// must be reachable; max_step + epsilon must not.
#[test]
fn cell_reachable_boundary_at_max_step() {
    let mut g = NavigationGraph::new(0.0, 10.0, 0.0, 10.0, 1.0);
    let idx = g.index(5, 5);
    g.heightfield[idx] = g.max_step;
    assert!(g.cell_reachable(5, 5), "exactly max_step: reachable");
    g.heightfield[idx] += 0.001;
    assert!(!g.cell_reachable(5, 5), "above max_step: unreachable");
}

/// The bake sources `max_step` from `scene.nav_settings`, not the hardcoded
/// constant (#276): a 1.0-high ledge is unreachable at the default step (0.5) but
/// becomes reachable once the scene's `max_step` is raised past it and we re-bake.
#[test]
fn bake_sources_max_step_from_scene_settings() {
    let mut scene = Scene::new();
    // Box footprint covers only cell (4,4); a 1.0 jump from every neighbour.
    add_box(
        &mut scene,
        "ledge",
        Vec3::new(3.6, 0.0, 3.6),
        Vec3::new(4.4, 1.0, 4.4),
    );
    let mut g = NavigationGraph::new(0.0, 10.0, 0.0, 10.0, 1.0);
    g.bake(&scene);
    assert!(
        !g.cell_reachable(4, 4),
        "1.0 ledge blocked at default step 0.5"
    );

    scene.nav_settings.max_step = 1.5;
    g.bake(&scene);
    assert_eq!(
        g.max_step, 1.5,
        "bake pulled max_step from the scene settings"
    );
    assert!(
        g.cell_reachable(4, 4),
        "ledge reachable once step is raised"
    );
}

/// The bake re-shapes the grid from `scene.nav_settings.grid_spacing` (#276):
/// a coarser spacing yields fewer cells against the same bounds.
#[test]
fn bake_reshapes_grid_from_grid_spacing() {
    let mut scene = Scene::new();
    let mut g = NavigationGraph::new(0.0, 10.0, 0.0, 10.0, 1.0);
    g.bake(&scene);
    let cells_unit = g.walkability.len();
    assert_eq!((g.width, g.height), (11, 11));

    scene.nav_settings.grid_spacing = 2.0;
    g.bake(&scene);
    assert_eq!(g.grid_spacing, 2.0);
    assert_eq!((g.width, g.height), (6, 6), "10/2 + 1 cells per axis");
    assert!(
        g.walkability.len() < cells_unit,
        "coarser spacing => fewer cells"
    );
}

/// Determinism with non-default settings (#276): a custom step/slope/spacing must
/// still produce a byte-identical bake across runs (the guarded sim must be a pure
/// function of the scene + settings).
#[test]
fn bake_with_custom_settings_is_deterministic() {
    let make = || {
        let mut scene = Scene::new();
        add_box(
            &mut scene,
            "ledge",
            Vec3::new(3.6, 0.0, 3.6),
            Vec3::new(4.4, 0.7, 4.4),
        );
        scene.nav_settings.max_step = 0.8;
        scene.nav_settings.max_slope = 0.6;
        scene.nav_settings.grid_spacing = 0.5;
        let mut g = NavigationGraph::new(0.0, 10.0, 0.0, 10.0, 1.0);
        g.bake(&scene);
        g
    };
    let a = make();
    let b = make();
    assert_eq!(a.heightfield, b.heightfield);
    assert_eq!(a.walkability, b.walkability);
    assert_eq!((a.width, a.height), (b.width, b.height));
    assert_eq!((a.max_step, a.max_slope), (b.max_step, b.max_slope));
}
