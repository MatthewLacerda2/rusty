//! Tests for the agent-height clearance pass (`headroom.rs`, #278): low overhangs carve
//! the cells beneath them, taller agents carve a superset (monotonic), high/absent
//! overhead is an exact no-op (back-compat), and the pass is deterministic. Mirrors the
//! `bake_tests.rs`/`erosion_tests.rs` `add_box` helper style.

use super::*;
use crate::scene::{ColliderComponent, ColliderShape, Scene};
use glam::Vec3;

/// Adds a static box collider spanning the given world AABB to `scene` (mirrors the
/// `add_box` helpers in the sibling navigation test files).
fn add_box(scene: &mut Scene, name: &str, min: Vec3, max: Vec3) {
    let id = scene.add_entity(name.to_string());
    let mut e = scene.get_entity_mut(id).expect("entity exists");
    e.is_static = true;
    e.collider = Some(ColliderComponent {
        active: true,
        shape: ColliderShape::Box { size: max - min },
        is_trigger: false,
        aabb_min: min,
        aabb_max: max,
    });
}

/// A floating ceiling slab over `x = x0..=x1`, `z = z0..=z1`, spanning `y = bottom..top`.
/// Its underside (`bottom`) is the overhead the headroom pass measures clearance to. It
/// sits above the ground (y = 0) so it never raises the floor it shadows.
fn add_overhang(scene: &mut Scene, x0: f32, x1: f32, z0: f32, z1: f32, bottom: f32) {
    add_box(
        scene,
        "overhang",
        Vec3::new(x0, bottom, z0),
        Vec3::new(x1, bottom + 0.5, z1),
    );
}

/// Bake a fresh 0..10 × 0..10 unit-spacing graph for `scene` at the given agent height.
fn baked(scene: &mut Scene, agent_height: f32) -> NavigationGraph {
    scene.nav_settings.agent_height = agent_height;
    let mut g = NavigationGraph::new(0.0, 10.0, 0.0, 10.0, 1.0);
    g.bake(scene);
    g
}

/// A low overhang (underside at y = 1.0, agent height 2.0) carves out the floor cells
/// beneath it; cells outside the overhang's footprint stay walkable.
#[test]
fn low_overhang_carves_cells_beneath() {
    let mut scene = Scene::new();
    // Slab covering cell columns x = 3..=5 at z = 4..=6, underside 1.0 above the floor.
    add_overhang(&mut scene, 2.6, 5.4, 3.6, 6.4, 1.0);
    // agent_radius 0 so erosion doesn't blur the result — isolate the headroom pass.
    scene.nav_settings.agent_radius = 0.0;
    let g = baked(&mut scene, 2.0);

    for gz in 4..=6 {
        for gx in 3..=5 {
            assert!(
                !g.is_walkable(gx, gz),
                "cell ({gx},{gz}) under a 1.0 overhang carved at agent height 2.0"
            );
        }
    }
    // Outside the footprint stays walkable.
    assert!(g.is_walkable(0, 0), "far corner untouched");
    assert!(
        g.is_walkable(8, 8),
        "cell well clear of the overhang walkable"
    );
}

/// Raising agent height shrinks the walkable set monotonically: a taller agent carves a
/// superset of a shorter one's holes on the same scene.
#[test]
fn taller_agent_carves_superset() {
    let make = |h: f32| {
        let mut scene = Scene::new();
        add_overhang(&mut scene, 2.6, 7.4, 2.6, 7.4, 1.5);
        scene.nav_settings.agent_radius = 0.0;
        baked(&mut scene, h)
    };
    let short = make(1.0); // headroom 1.5 >= 1.0 → nothing carved
    let tall = make(2.0); // headroom 1.5 < 2.0 → carves under the slab

    for (idx, &short_walk) in short.walkability.iter().enumerate() {
        if !short_walk {
            assert!(
                !tall.walkability[idx],
                "cell {idx} carved for the short agent must stay carved for the tall one"
            );
        }
    }
    assert!(
        tall.walkability.iter().filter(|&&w| !w).count()
            > short.walkability.iter().filter(|&&w| !w).count(),
        "the taller agent carves strictly more cells under a 1.5-high overhang"
    );
}

/// An overhang higher than the agent carves nothing — clearance above the agent is fine.
#[test]
fn high_overhang_is_no_op() {
    let mut scene = Scene::new();
    add_overhang(&mut scene, 2.6, 7.4, 2.6, 7.4, 3.0); // underside 3.0 > agent height 2.0
    scene.nav_settings.agent_radius = 0.0;
    let g = baked(&mut scene, 2.0);
    assert!(
        g.walkability.iter().all(|&w| w),
        "headroom 3.0 clears a 2.0 agent everywhere: no cell carved"
    );
}

/// Boundary: a cell whose clearance is *exactly* the agent height stays walkable — the
/// agent just fits, since the carve threshold is "below the height" (`< agent_height`, not
/// `<=`). An overhang underside at y = 2.0 over flat ground (support 0.0) ⇒ clearance
/// exactly 2.0, equal to the agent height. Pins the `<` so a `<=` mutant (which would carve
/// a cell the agent exactly clears) cannot survive.
#[test]
fn clearance_exactly_equal_to_height_stays_walkable() {
    let mut scene = Scene::new();
    add_overhang(&mut scene, 2.6, 7.4, 2.6, 7.4, 2.0); // underside 2.0 == agent height 2.0
    scene.nav_settings.agent_radius = 0.0;
    let g = baked(&mut scene, 2.0);
    for gz in 3..=6 {
        for gx in 3..=6 {
            assert!(
                g.is_walkable(gx, gz),
                "cell ({gx},{gz}) with clearance exactly 2.0 fits a 2.0 agent: not carved"
            );
        }
    }
}

/// A scene with no overhead geometry bakes identically with the feature on (default
/// height) and off (height 0) — the back-compat proof that simple scenes are untouched.
#[test]
fn no_overhead_bakes_identically_to_feature_off() {
    let make = |h: f32| {
        let mut scene = Scene::new();
        // A floor box and a tall wall — geometry beside the agent, never overhead.
        add_box(
            &mut scene,
            "wall",
            Vec3::new(4.6, 0.0, 0.0),
            Vec3::new(5.4, 3.0, 10.0),
        );
        baked(&mut scene, h)
    };
    let off = make(0.0); // feature off
    let on = make(2.0); // default agent height
    assert_eq!(
        off.walkability, on.walkability,
        "no overhead ⇒ headroom pass is a no-op vs. height 0"
    );
}

/// The collider that *defines* a cell's floor (its top == floor, bottom below) must never
/// count as that cell's own ceiling — a raised platform with high clearance stays walkable.
#[test]
fn floor_defining_collider_is_not_its_own_ceiling() {
    let mut scene = Scene::new();
    // A 1.0-high platform over x = 2..=7, z = 2..=7: it raises the floor to y = 1.0, and
    // its bottom (y = 0.0) sits below that floor, so it is not overhead for its own cells.
    add_box(
        &mut scene,
        "platform",
        Vec3::new(2.6, 0.0, 2.6),
        Vec3::new(7.4, 1.0, 7.4),
    );
    scene.nav_settings.agent_radius = 0.0;
    let g = baked(&mut scene, 2.0);
    assert_eq!(g.height_at(4, 4), 1.0, "platform raised the floor");
    assert!(
        g.is_walkable(4, 4),
        "platform top has open sky above: not carved by its own slab"
    );
}

/// Determinism: same scene + agent height → byte-identical walkability across runs.
#[test]
fn headroom_is_deterministic_across_runs() {
    let make = || {
        let mut scene = Scene::new();
        add_overhang(&mut scene, 2.6, 6.4, 2.6, 6.4, 1.0);
        baked(&mut scene, 2.0)
    };
    let a = make();
    let b = make();
    assert_eq!(
        a.walkability, b.walkability,
        "headroom carve must be stable"
    );
    assert_eq!(a.heightfield, b.heightfield);
}
