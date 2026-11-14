//! The small-object cull's verdicts (#1714), adapter-free.

use glam::{Mat4, Quat, Vec3};

use super::SmallObjectCull;
use rusty_components::{MeshComponent, Projection};
use rusty_scene::Camera;

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

/// A unit cube's local bounds.
const UNIT: (Vec3, Vec3) = (Vec3::splat(-0.5), Vec3::splat(0.5));

/// Whether `cull` culls `m`, a unit cube centred `d` metres down -Z.
fn culls_at(cull: &SmallObjectCull, m: &MeshComponent, d: f32) -> bool {
    let c = Vec3::new(0.0, 0.0, -d);
    cull.culls(m, c, UNIT, Mat4::from_translation(c))
}

#[test]
fn both_settings_off_cull_nothing_at_any_distance() {
    let cull = SmallObjectCull::from_camera(&camera());
    for d in [0.0, 1.0, 1e4, 1e30] {
        assert!(!culls_at(&cull, &mesh(0.0, 0.0), d), "{d}");
    }
}

#[test]
fn max_draw_distance_culls_past_it_measured_to_the_bounds_centre() {
    let cull = SmallObjectCull::from_camera(&camera());
    let m = mesh(10.0, 0.0);
    assert!(!culls_at(&cull, &m, 9.9));
    assert!(!culls_at(&cull, &m, 10.0), "at the distance is still drawn");
    assert!(culls_at(&cull, &m, 10.1));
}

#[test]
fn min_screen_size_is_unitys_relative_height() {
    let cull = SmallObjectCull::from_camera(&camera());
    // 1 m at d covers 1/(2d): 0.05 at 10 m, 0.025 at 20 m.
    let m = mesh(0.0, 0.04);
    assert!(!culls_at(&cull, &m, 10.0));
    assert!(culls_at(&cull, &m, 20.0));
    // A camera inside the bounds sees it infinitely large.
    assert!(!culls_at(&cull, &mesh(0.0, 1.0), 0.0));
}

#[test]
fn an_orthographic_view_measures_size_whatever_the_distance() {
    let mut cam = camera();
    cam.projection = Projection::Orthographic { size: 5.0 };
    let cull = SmallObjectCull::from_camera(&cam);
    // 1 m of a 10 m tall view is 0.1, near or far.
    for d in [1.0, 1000.0] {
        assert!(!culls_at(&cull, &mesh(0.0, 0.09), d));
        assert!(culls_at(&cull, &mesh(0.0, 0.11), d));
    }
}

#[test]
fn a_capture_culls_nothing() {
    let off = SmallObjectCull::off();
    assert!(!culls_at(&off, &mesh(1.0, 0.9), 100.0));
}

#[test]
fn a_nan_or_negative_setting_from_a_file_culls_nothing() {
    let cull = SmallObjectCull::from_camera(&camera());
    for v in [f32::NAN, -5.0] {
        assert!(!culls_at(&cull, &mesh(v, 0.0), 100.0), "{v}");
        assert!(!culls_at(&cull, &mesh(0.0, v), 100.0), "{v}");
    }
}

#[test]
fn a_spinning_or_scaled_prop_keeps_one_size() {
    let cull = SmallObjectCull::from_camera(&camera());
    // A 1 m cube at 10 m covers 0.05; its world box's diagonal would be 0.0707 at 45°.
    let m = mesh(0.0, 0.06);
    let c = Vec3::new(0.0, 0.0, -10.0);
    let verdicts: Vec<bool> = (0..32)
        .map(|i| {
            let spin = Quat::from_rotation_y(i as f32 * std::f32::consts::TAU / 32.0);
            let world = Mat4::from_rotation_translation(spin, c);
            cull.culls(&m, c, UNIT, world)
        })
        .collect();
    assert!(verdicts.iter().all(|&v| v), "{verdicts:?}");
    // Scaled ×2 it covers 0.1: drawn.
    let world = Mat4::from_scale_rotation_translation(Vec3::splat(2.0), Quat::IDENTITY, c);
    assert!(!cull.culls(&m, c, UNIT, world));
}
