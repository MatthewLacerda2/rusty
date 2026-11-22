use super::super::super::test_support::{add_box, add_floor, add_rotated_box, bake_pinned};
use super::super::super::NavigationGraph;
use glam::{Quat, Vec3};
use rusty_scene::Scene;

const GRADE: f32 = 0.3;
const THICK: f32 = 0.4;

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
