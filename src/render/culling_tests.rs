//! Tests for view-frustum culling (#330): plane extraction, the AABB test, the local→
//! world AABB transform, and the shadow contract that a caster behind the camera is kept
//! by the LIGHT's frustum even though the camera's frustum rejects it.

use super::{transformed_aabb, Frustum};
use glam::{Mat4, Vec3};

/// A camera at `(0, 0, 5)` looking down −Z (visible region is z < 5), 90° fov.
fn camera_vp() -> Mat4 {
    let view = Mat4::look_at_rh(Vec3::new(0.0, 0.0, 5.0), Vec3::ZERO, Vec3::Y);
    let proj = Mat4::perspective_rh(90.0_f32.to_radians(), 1.0, 0.1, 100.0);
    proj * view
}

/// The directional-light ortho volume, mirroring `ShadowRenderer::update_light_space`:
/// look at the origin from 45 units up the light direction, ±30 ortho half-extent.
fn light_vp() -> Mat4 {
    let dir = Vec3::new(-0.5, -1.0, -0.3).normalize();
    let view = Mat4::look_at_rh(-dir * 45.0, Vec3::ZERO, Vec3::Y);
    let proj = Mat4::orthographic_rh(-30.0, 30.0, -30.0, 30.0, 1.0, 100.0);
    proj * view
}

/// A unit box centered at `c`.
fn unit_box(c: Vec3) -> (Vec3, Vec3) {
    (c - Vec3::splat(0.5), c + Vec3::splat(0.5))
}

#[test]
fn box_in_front_of_camera_is_kept() {
    let f = Frustum::from_view_projection(camera_vp());
    let (min, max) = unit_box(Vec3::ZERO);
    assert!(f.intersects_aabb(min, max));
}

#[test]
fn box_far_to_the_side_is_culled() {
    let f = Frustum::from_view_projection(camera_vp());
    let (min, max) = unit_box(Vec3::new(1000.0, 0.0, 0.0));
    assert!(!f.intersects_aabb(min, max));
}

#[test]
fn box_behind_camera_is_culled() {
    let f = Frustum::from_view_projection(camera_vp());
    // z = 20 is behind the eye at z = 5 (camera looks toward −Z).
    let (min, max) = unit_box(Vec3::new(0.0, 0.0, 20.0));
    assert!(!f.intersects_aabb(min, max));
}

/// The shadow-pass contract (#330): a caster behind the camera still throws a shadow into
/// view, so shadow collects must cull against the LIGHT's frustum, not the camera's. The
/// same box the camera rejects is kept by the light's ortho volume.
#[test]
fn caster_behind_camera_survives_light_frustum() {
    let (min, max) = unit_box(Vec3::new(0.0, 0.0, 20.0));
    let camera = Frustum::from_view_projection(camera_vp());
    let light = Frustum::from_view_projection(light_vp());
    assert!(!camera.intersects_aabb(min, max), "camera should reject the off-screen caster");
    assert!(light.intersects_aabb(min, max), "light must keep the caster that shadows the view");
}

#[test]
fn transformed_aabb_follows_world_translation() {
    let (lo, hi) = (Vec3::splat(-1.0), Vec3::splat(1.0));
    let (min, max) = transformed_aabb(lo, hi, Mat4::from_translation(Vec3::new(10.0, 0.0, 0.0)));
    assert_eq!(min, Vec3::new(9.0, -1.0, -1.0));
    assert_eq!(max, Vec3::new(11.0, 1.0, 1.0));
}

/// A local box that sits inside the frustum only AFTER its world transform pushes it out
/// is correctly culled once transformed — proves the cull tests the world AABB, not local.
#[test]
fn cull_uses_world_transformed_aabb() {
    let f = Frustum::from_view_projection(camera_vp());
    let (lo, hi) = unit_box(Vec3::ZERO);
    // In local space it's at the origin (in view); translate it far behind the camera.
    let (min, max) = transformed_aabb(lo, hi, Mat4::from_translation(Vec3::new(0.0, 0.0, 40.0)));
    assert!(!f.intersects_aabb(min, max));
}
