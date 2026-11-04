//! The small-object cull's verdicts (#1714), adapter-free.

use glam::Vec3;

use super::SmallObjectCull;
use crate::components::{MeshComponent, Projection};
use crate::scene::Camera;

/// A camera at the origin with a 90° vertical field of view: at distance `d` the view
/// is `2d` metres tall, so a 1 m prop covers `1 / (2d)` of it.
fn camera() -> Camera {
    let mut cam = Camera::new(Vec3::ZERO, 0.0, 0.0);
    cam.fov = 90.0;
    cam
}

fn mesh(max_draw_distance: f32, min_screen_size: f32) -> MeshComponent {
    MeshComponent {
        max_draw_distance,
        min_screen_size,
        ..MeshComponent::new("Box", Vec::new(), Vec::new())
    }
}

/// A unit cube centred `d` metres down -Z.
fn cube_at(d: f32) -> (Vec3, Vec3) {
    let c = Vec3::new(0.0, 0.0, -d);
    (c - Vec3::splat(0.5), c + Vec3::splat(0.5))
}

#[test]
fn both_settings_off_cull_nothing_at_any_distance() {
    let cull = SmallObjectCull::from_camera(&camera());
    for d in [0.0, 1.0, 1e4, 1e30] {
        assert!(!cull.culls(&mesh(0.0, 0.0), cube_at(d)), "{d}");
    }
}

#[test]
fn max_draw_distance_culls_past_it_measured_to_the_bounds_centre() {
    let cull = SmallObjectCull::from_camera(&camera());
    let m = mesh(10.0, 0.0);
    assert!(!cull.culls(&m, cube_at(9.9)));
    assert!(
        !cull.culls(&m, cube_at(10.0)),
        "at the distance is still drawn"
    );
    assert!(cull.culls(&m, cube_at(10.1)));
}

#[test]
fn min_screen_size_is_unitys_relative_height() {
    let cull = SmallObjectCull::from_camera(&camera());
    // 1 m at d covers 1/(2d): 0.05 at 10 m, 0.025 at 20 m.
    let m = mesh(0.0, 0.04);
    assert!(!cull.culls(&m, cube_at(10.0)));
    assert!(cull.culls(&m, cube_at(20.0)));
    // A camera inside the bounds sees it infinitely large.
    assert!(!cull.culls(&mesh(0.0, 1.0), cube_at(0.0)));
}

#[test]
fn an_orthographic_view_measures_size_whatever_the_distance() {
    let mut cam = camera();
    cam.projection = Projection::Orthographic { size: 5.0 };
    let cull = SmallObjectCull::from_camera(&cam);
    // 1 m of a 10 m tall view is 0.1, near or far.
    for d in [1.0, 1000.0] {
        assert!(!cull.culls(&mesh(0.0, 0.09), cube_at(d)));
        assert!(cull.culls(&mesh(0.0, 0.11), cube_at(d)));
    }
}

#[test]
fn a_capture_culls_nothing() {
    let off = SmallObjectCull::off();
    assert!(!off.culls(&mesh(1.0, 0.9), cube_at(100.0)));
}

#[test]
fn a_nan_or_negative_setting_from_a_file_culls_nothing() {
    let cull = SmallObjectCull::from_camera(&camera());
    for v in [f32::NAN, -5.0] {
        assert!(!cull.culls(&mesh(v, 0.0), cube_at(100.0)), "{v}");
        assert!(!cull.culls(&mesh(0.0, v), cube_at(100.0)), "{v}");
    }
}
