//! A ramp's surface (#781): the height an agent stands at follows the slope, not its
//! cells' highest points, and the creases at the foot and the top come out exact.

use super::super::super::test_support::{add_box, add_floor, add_rotated_box, bake_pinned};
use super::super::super::NavigationGraph;
use crate::scene::Scene;
use glam::{Quat, Vec3};

const GRADE: f32 = 0.3;
const THICK: f32 = 0.4;

/// A floor at y = 0, a 0.3-grade ramp up +x from x = 2 to x = 8 (z 3..7), and a
/// platform at its top from x = 8 on.
fn ramp() -> NavigationGraph {
    let mut scene = Scene::new();
    add_floor(&mut scene, 0.0, 12.0, 0.0, 10.0);
    let angle = GRADE.atan();
    let length = (6.0f32).hypot(6.0 * GRADE);
    let normal = Vec3::new(-angle.sin(), angle.cos(), 0.0);
    let top_centre = Vec3::new(5.0, 3.0 * GRADE, 5.0);
    add_rotated_box(
        &mut scene,
        top_centre - normal * (THICK / 2.0),
        Vec3::new(length, THICK, 4.0),
        Quat::from_rotation_z(angle),
        true,
    );
    add_box(
        &mut scene,
        Vec3::new(8.0, 0.0, 3.0),
        Vec3::new(12.0, 6.0 * GRADE, 7.0),
    );
    scene.nav_settings.agent_radius = 0.0;
    bake_pinned(&mut scene)
}

/// The ground the scene above is built with, at `x`.
fn truth(x: f32) -> f32 {
    (GRADE * (x - 2.0)).clamp(0.0, 6.0 * GRADE)
}

#[test]
fn an_agent_on_a_ramp_stands_on_its_surface() {
    let g = ramp();
    for i in 0..=100 {
        let x = 0.5 + i as f32 * 0.1;
        let feet = Vec3::new(x, truth(x), 5.0);
        let on = g.span_under(feet).expect("walkable all the way up");
        let y = g.surface_y(on, x, 5.0);
        assert!(
            (y - truth(x)).abs() < 1e-3,
            "at x = {x}: stands at {y}, the ground is at {}",
            truth(x)
        );
    }
}

#[test]
fn span_tops_stay_their_cells_highest_point() {
    let g = ramp();
    let on = g.span_under(Vec3::new(5.0, truth(5.0), 5.0)).unwrap();
    let top = g.spans[on.index as usize].y;
    assert!(
        (top - truth(5.5)).abs() < 1e-3,
        "connectivity still compares tops: {top}"
    );
}
