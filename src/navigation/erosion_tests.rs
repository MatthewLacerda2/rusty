//! Tests for the agent-radius erosion pass (`bake.rs`, #277), kept in this `_tests.rs`
//! sibling so the source file stays under the size cap. Covers the thin-passage
//! rejection, the wide-corridor clearance (pulled off both walls), the `radius == 0`
//! no-op, and determinism — mirroring the `bake_tests.rs` helper style.

use super::*;
use crate::scene::{ColliderComponent, ColliderShape, Scene};
use glam::Vec3;

/// Adds a static box collider spanning the given world AABB to `scene` (mirrors the
/// `add_box` helper in `bake_tests.rs`).
fn add_box(scene: &mut Scene, name: &str, min: Vec3, max: Vec3) {
    let id = scene.add_entity(name.to_string());
    scene.world.set_static(id, true);
    scene.world.set_collider(id, Some(ColliderComponent {
        active: true,
        shape: ColliderShape::Box { size: max - min },
        is_trigger: false,
        aabb_min: min,
        aabb_max: max,
    });
}

/// A tall (y=3) wall strip running along z over `z = z0..=z1` at integer column `gx`.
/// The footprint `[gx-0.25, gx+0.25]` covers exactly the cells in that column, raising
/// them far above their neighbours so they bake as unreachable obstacles (wall tops).
fn add_wall_strip(scene: &mut Scene, name: &str, gx: i32, z0: f32, z1: f32) {
    let x = gx as f32;
    add_box(
        scene,
        name,
        Vec3::new(x - 0.25, 0.0, z0),
        Vec3::new(x + 0.25, 3.0, z1),
    );
}

/// A corridor: two tall wall columns at `left` and `right` with open floor between,
/// running the full z extent. Baked at unit spacing on a 0..=10 grid with `radius`.
fn corridor_graph(left: i32, right: i32, radius: f32) -> NavigationGraph {
    let mut scene = Scene::new();
    add_wall_strip(&mut scene, "left", left, 0.0, 10.0);
    add_wall_strip(&mut scene, "right", right, 0.0, 10.0);
    scene.nav_settings.agent_radius = radius;
    let mut graph = NavigationGraph::new(0.0, 10.0, 0.0, 10.0, 1.0);
    graph.bake(&scene);
    graph
}

/// A 1-cell-wide passage (walls at x=3 and x=5, floor at x=4) is narrower than
/// `2*radius` (radius 1.0 → diameter 2.0 > the single open cell), so erosion closes it:
/// the lone floor column becomes non-walkable and no longer routes through.
#[test]
fn thin_passage_closes_under_erosion() {
    let graph = corridor_graph(3, 5, 1.0);
    for gz in 0..graph.height {
        assert!(
            !graph.is_walkable(4, gz),
            "narrow corridor cell (4,{gz}) eroded shut"
        );
    }
    // The two sides are now disconnected: A* cannot thread the closed column, so any
    // path it returns from the left island stays left of the wall line and never reaches
    // the right-hand target column (it can't step onto the eroded x=4 cells).
    if let Some(path) = graph.find_path(1, 5, 8, 5) {
        assert!(
            path.iter().all(|&(x, _)| x != 4),
            "no path step lands on the closed passage column x=4"
        );
        assert!(
            path.iter().all(|&(x, _)| x < 5),
            "path cannot cross to the right side of the wall line"
        );
    }
}

/// A wide corridor (walls at x=1 and x=9, open x=2..=8) keeps a walkable core but is
/// pulled OFF BOTH walls by one cell (radius 1.0): the cells flush against each wall
/// erode, while the centre stays walkable — clearance, not wall-hugging.
#[test]
fn wide_corridor_keeps_core_pulled_off_both_walls() {
    let graph = corridor_graph(1, 9, 1.0);
    let gz = 5;
    // Pulled off the left wall (x=1): the adjacent floor cell x=2 erodes.
    assert!(!graph.is_walkable(2, gz), "cell flush to left wall eroded");
    // Pulled off the right wall (x=9): the adjacent floor cell x=8 erodes.
    assert!(!graph.is_walkable(8, gz), "cell flush to right wall eroded");
    // The core (x=3..=7) survives — a walkable channel remains.
    for gx in 3..=7 {
        assert!(graph.is_walkable(gx, gz), "core cell ({gx},{gz}) walkable");
    }
}

/// `agent_radius == 0` is an EXACT no-op: byte-identical walkability to a bake of the
/// same scene with erosion effectively disabled. We compare against an explicit
/// erosion-off baseline (the pre-#277 behaviour: walkability all-true).
#[test]
fn radius_zero_is_exact_no_op() {
    let mut scene = Scene::new();
    add_wall_strip(&mut scene, "left", 3, 0.0, 10.0);
    add_wall_strip(&mut scene, "right", 5, 0.0, 10.0);

    scene.nav_settings.agent_radius = 0.0;
    let mut zero = NavigationGraph::new(0.0, 10.0, 0.0, 10.0, 1.0);
    zero.bake(&scene);

    // Pre-#277 behaviour: the bake never wrote walkability, so it stays all-true.
    assert!(
        zero.walkability.iter().all(|&w| w),
        "radius 0 leaves walkability untouched (all true), matching pre-erosion bake"
    );
}

/// A non-zero radius DOES erode (contrast with the no-op above), proving the pass is
/// wired to `agent_radius` and not dead. With `ceil` rounding, any radius > 0 yields at
/// least one cell of erosion (a sub-grid 0.4 still ceils to 1 cell — the conservative
/// rule that never leaves an agent clipping a wall).
#[test]
fn nonzero_radius_erodes_some_cells() {
    let eroded = corridor_graph(3, 5, 0.4);
    assert!(
        eroded.walkability.iter().any(|&w| !w),
        "a sub-grid radius still ceils to one cell and erodes"
    );
}

/// Determinism: same scene + radius → byte-identical walkability across runs (the
/// guarded sim must be a pure function of scene + settings).
#[test]
fn erosion_is_deterministic_across_runs() {
    let a = corridor_graph(2, 8, 1.0);
    let b = corridor_graph(2, 8, 1.0);
    assert_eq!(a.walkability, b.walkability, "erosion must be stable");
    assert_eq!(a.heightfield, b.heightfield);
}

/// A larger radius erodes at least as much as a smaller one (monotonic clearance):
/// radius 2.0 must clear a superset of the cells radius 1.0 clears, on the same scene.
#[test]
fn larger_radius_erodes_superset() {
    let small = corridor_graph(1, 9, 1.0);
    let large = corridor_graph(1, 9, 2.0);
    for (idx, &small_walk) in small.walkability.iter().enumerate() {
        if !small_walk {
            assert!(
                !large.walkability[idx],
                "cell {idx} cleared at r=1 must stay cleared at r=2"
            );
        }
    }
    assert!(
        large.walkability.iter().filter(|&&w| !w).count()
            >= small.walkability.iter().filter(|&&w| !w).count(),
        "larger radius erodes at least as many cells"
    );
}
