//! The outline's scissor (#1175): the selection's bounds on screen, padded by the
//! ring, clamped to the view, the whole view when they reach behind the camera, and
//! nothing when they lie outside it. No GPU.

use glam::{Mat4, Vec3};

use super::bounds::OutlineBounds;
use crate::scene::Camera;

/// A square view looking down -Z from 10 units away, through a 90° lens.
fn view_proj() -> Mat4 {
    let mut cam = Camera::new(Vec3::Z * 10.0, -90.0, 0.0);
    cam.fov = 90.0;
    cam.build_view_projection(1.0)
}

#[test]
fn a_box_ahead_covers_its_projection_padded() {
    // A flat 2 x 2 square at the origin spans 10 px of a 100 px view, centred.
    let square = OutlineBounds::Box(Vec3::new(-1.0, -1.0, 0.0), Vec3::new(1.0, 1.0, 0.0));
    assert_eq!(
        square.screen_rect(view_proj(), (100, 100), 0),
        Some([45, 45, 10, 10])
    );
    assert_eq!(
        square.screen_rect(view_proj(), (100, 100), 3),
        Some([42, 42, 16, 16])
    );
}

#[test]
fn a_box_is_clamped_to_the_view_and_dropped_when_outside_it() {
    let wide = OutlineBounds::Box(Vec3::new(-30.0, -1.0, 0.0), Vec3::new(1.0, 1.0, 0.0));
    assert_eq!(
        wide.screen_rect(view_proj(), (100, 100), 0),
        Some([0, 45, 55, 10])
    );
    let aside = OutlineBounds::Box(Vec3::new(40.0, -1.0, 0.0), Vec3::new(41.0, 1.0, 0.0));
    assert_eq!(aside.screen_rect(view_proj(), (100, 100), 2), None);
}

#[test]
fn behind_the_camera_or_unbounded_covers_the_whole_view() {
    let through = OutlineBounds::Box(Vec3::new(-1.0, -1.0, 0.0), Vec3::new(1.0, 1.0, 20.0));
    let whole = Some([0, 0, 100, 100]);
    assert_eq!(through.screen_rect(view_proj(), (100, 100), 0), whole);
    let everywhere = OutlineBounds::Everywhere;
    assert_eq!(everywhere.screen_rect(view_proj(), (100, 100), 0), whole);
}
