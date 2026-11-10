//! Steps and stairs join at every grid (#2001): a step is held to `max_step` alone,
//! Recast's `walkableClimb`, so a finer grid never disconnects what a coarser one
//! joins. The scenes rebuild the tower-bar case on primitives: a 0.15 m platform
//! with a nosing lip, and a 14-riser stair of 0.17 m risers on 0.27 m treads.

use super::super::super::test_support::{add_box, add_floor, add_rotated_box, arrives};
use super::super::super::{NavBounds, NavigationGraph};
use crate::scene::Scene;
use glam::{Quat, Vec3};

const RADII: [f32; 4] = [0.15, 0.2, 0.25, 0.3];
const GRIDS: [f32; 4] = [0.1, 0.125, 0.15, 0.2];
const RISER: f32 = 0.17;
const TREAD: f32 = 0.27;
const STEPS: usize = 14;

fn bake(scene: &mut Scene, radius: f32, grid: f32) -> NavigationGraph {
    let s = &mut scene.nav_settings;
    s.bounds = Some(NavBounds::new(-1.0, 7.0, -1.0, 7.0));
    (s.agent_radius, s.grid_spacing) = (radius, grid);
    (s.max_step, s.max_slope, s.agent_height) = (0.5, 1.0, 2.0);
    NavigationGraph::from_scene(scene)
}

/// The `(radius, grid)` pairs at which a walk from `from` to `to` does not arrive;
/// empty when it arrives at every one. Both probe points must have a span under
/// them at every bake, so a vanished floor fails instead of counting as refused.
fn joins(scene: &mut Scene, from: Vec3, to: Vec3) -> Vec<(f32, f32)> {
    let mut refused = Vec::new();
    for r in RADII {
        for grid in GRIDS {
            let g = bake(scene, r, grid);
            let a = g
                .span_under(from)
                .unwrap_or_else(|| panic!("no span at {from} ({r}, {grid})"));
            let b = g
                .span_under(to)
                .unwrap_or_else(|| panic!("no span at {to} ({r}, {grid})"));
            if !arrives(&g, a, b) {
                refused.push((r, grid));
            }
        }
    }
    refused
}

/// A floor, and a platform `rise` up over x 3..6; `lip` adds a 5 cm nosing whose
/// underside is 0.045 below the platform top.
fn platform(rise: f32, lip: bool) -> Scene {
    let mut scene = Scene::new();
    add_floor(&mut scene, 0.0, 6.0, 0.0, 3.0);
    add_box(
        &mut scene,
        Vec3::new(3.0, 0.0, 0.0),
        Vec3::new(6.0, rise, 3.0),
    );
    if lip {
        let under = rise - 0.045;
        add_box(
            &mut scene,
            Vec3::new(2.95, under, 0.0),
            Vec3::new(3.0, rise, 3.0),
        );
    }
    scene
}

#[test]
fn a_platform_one_riser_up_joins_the_floor_at_every_grid() {
    let mut scene = platform(0.15, true);
    let (up, down) = (Vec3::new(5.0, 0.15, 1.5), Vec3::new(1.0, 0.0, 1.5));
    assert_eq!(joins(&mut scene, up, down), [], "platform -> floor");
    assert_eq!(joins(&mut scene, down, up), [], "floor -> platform");
}

#[test]
fn a_step_over_max_step_stays_apart() {
    let mut scene = platform(0.6, false);
    let (up, down) = (Vec3::new(5.0, 0.6, 1.5), Vec3::new(1.0, 0.0, 1.5));
    // Climbing: no drop link helps, and 0.6 is past `max_step` at every grid.
    let refused = joins(&mut scene, down, up);
    assert_eq!(refused.len(), RADII.len() * GRIDS.len(), "{refused:?}");
}

/// A straight stair down +x: riser `i` (1-based) tops at `i * RISER`, and the
/// landing past the last tread is `STEPS * RISER` up.
fn straight_stair() -> Scene {
    let mut scene = Scene::new();
    add_floor(&mut scene, -1.0, 7.0, 0.0, 1.5);
    for i in 1..=STEPS {
        let x = 1.0 + (i - 1) as f32 * TREAD;
        let top = i as f32 * RISER;
        add_box(&mut scene, Vec3::new(x, 0.0, 0.0), Vec3::new(7.0, top, 1.5));
    }
    scene
}

#[test]
fn a_straight_stair_joins_top_to_bottom_at_every_grid() {
    let mut scene = straight_stair();
    let top = Vec3::new(6.0, STEPS as f32 * RISER, 0.75);
    let bottom = Vec3::new(0.0, 0.0, 0.75);
    assert_eq!(joins(&mut scene, top, bottom), [], "top -> bottom");
    assert_eq!(joins(&mut scene, bottom, top), [], "bottom -> top");
}

/// A curved stair: each tread a box turned about the y axis at (0, 0), a 1.2 m
/// wide ring of radius 3 (treads `TREAD` deep on that line), rising from a floor;
/// each box is as deep as the outer edge's arc, so no gap opens between treads,
/// and a landing of six treads' length follows the last one.
fn curved_stair() -> (Scene, Vec3) {
    let mut scene = Scene::new();
    add_floor(&mut scene, -1.0, 7.0, -1.0, 7.0);
    let (radius, width) = (3.0, 1.2);
    let turn = TREAD / radius;
    let at = |angle: f32, y: f32| Vec3::new(radius * angle.cos(), y, radius * angle.sin());
    for i in 1..=STEPS + 6 {
        let angle = i as f32 * turn;
        let top = i.min(STEPS) as f32 * RISER;
        let size = Vec3::new(width, top, (radius + width) * turn);
        let rot = Quat::from_rotation_y(-angle);
        add_rotated_box(&mut scene, at(angle, top * 0.5), size, rot, true);
    }
    let landing = at((STEPS + 2) as f32 * turn, STEPS as f32 * RISER);
    (scene, landing)
}

#[test]
fn a_curved_stair_joins_top_to_bottom_at_every_grid() {
    let (mut scene, top) = curved_stair();
    let bottom = Vec3::new(3.0, 0.0, -0.6);
    assert_eq!(joins(&mut scene, top, bottom), [], "top -> bottom");
    assert_eq!(joins(&mut scene, bottom, top), [], "bottom -> top");
}
