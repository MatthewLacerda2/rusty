//! The 0.25 m default cell (#1887): passages the old 1 m cell closed stay open.
//!
//! What fits through a gap is the cell *and* the agent radius: erosion drops every
//! span within the radius of an edge (`erosion`), so at the default 0.5 m radius no
//! gap under about 1 m is walkable at any cell, by design. So the 0.6 m gap is
//! measured with no radius (the cell alone decides), and a 1.4 m doorway, the
//! Dust2 bench level's, at the default radius.

use glam::Vec3;

use super::super::super::test_support::{add_box, add_floor, arrives, ground};
use super::super::super::{NavBounds, NavigationGraph};
use rusty_scene::nav_settings::{DEFAULT_AGENT_RADIUS, DEFAULT_GRID_SPACING};
use rusty_scene::Scene;

/// A 10 × 6 floor split along x = 5 by a wall with a `gap` wide opening at z = 3,
/// baked at `spacing` with `radius`; whether a path crosses it.
fn crosses(gap: f32, spacing: f32, radius: f32) -> bool {
    let mut scene = Scene::new();
    scene.nav_settings.bounds = Some(NavBounds::new(0.0, 10.0, 0.0, 6.0));
    (
        scene.nav_settings.grid_spacing,
        scene.nav_settings.agent_radius,
    ) = (spacing, radius);
    add_floor(&mut scene, -1.0, 11.0, -1.0, 7.0);
    let (lo, hi) = (3.0 - gap / 2.0, 3.0 + gap / 2.0);
    add_box(
        &mut scene,
        Vec3::new(4.8, 0.0, -1.0),
        Vec3::new(5.2, 3.0, lo),
    );
    add_box(
        &mut scene,
        Vec3::new(4.8, 0.0, hi),
        Vec3::new(5.2, 3.0, 7.0),
    );
    let mut g = NavigationGraph::from_scene(&scene);
    g.bake(&scene);
    let cell = |x: f32| (x / spacing).round() as i32;
    arrives(
        &g,
        ground(&g, cell(1.5), cell(3.0)),
        ground(&g, cell(8.5), cell(3.0)),
    )
}

#[test]
fn a_0_6_m_gap_is_walkable_at_the_default_cell_and_not_at_1_m() {
    assert_eq!(DEFAULT_GRID_SPACING, 0.25);
    assert!(crosses(0.6, DEFAULT_GRID_SPACING, 0.0));
    assert!(!crosses(0.6, 1.0, 0.0));
}

#[test]
fn a_1_4_m_doorway_is_open_at_the_default_cell_and_radius_and_shut_at_1_m() {
    assert!(crosses(1.4, DEFAULT_GRID_SPACING, DEFAULT_AGENT_RADIUS));
    assert!(!crosses(1.4, 1.0, DEFAULT_AGENT_RADIUS));
    assert!(
        !crosses(0.6, DEFAULT_GRID_SPACING, DEFAULT_AGENT_RADIUS),
        "too narrow"
    );
}
