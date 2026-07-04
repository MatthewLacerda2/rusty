//! Frustum extraction / AABB culling tests, including the pop-a-shadow guard (#330).

use super::{transform_aabb, Frustum};
use glam::{Mat4, Vec3};

/// A perspective camera at the origin looking down -Z, aspect 1, 60° vfov.
fn camera_frustum() -> Frustum {
    let view = Mat4::look_at_rh(Vec3::ZERO, Vec3::new(0.0, 0.0, -1.0), Vec3::Y);
    let proj = Mat4::perspective_rh(60_f32.to_radians(), 1.0, 0.1, 100.0);
    Frustum::from_view_proj(proj * view)
}

/// The directional light's ortho shadow volume, mirroring `ShadowRenderer::update_light_space`:
/// a 60×60 ortho box looking at the origin from `-dir * 45`.
fn light_frustum(light_dir: Vec3) -> Frustum {
    let dir = light_dir.normalize();
    let view = Mat4::look_at_rh(-dir * 45.0, Vec3::ZERO, Vec3::Y);
    let proj = Mat4::orthographic_rh(-30.0, 30.0, -30.0, 30.0, 1.0, 100.0);
    Frustum::from_view_proj(proj * view)
}

/// A small axis-aligned box centered at `c`.
fn box_at(c: Vec3) -> (Vec3, Vec3) {
    (c - Vec3::splat(0.5), c + Vec3::splat(0.5))
}

#[test]
fn keeps_geometry_in_front_culls_geometry_behind() {
    let f = camera_frustum();
    let (fmin, fmax) = box_at(Vec3::new(0.0, 0.0, -5.0));
    assert!(
        f.intersects_aabb(fmin, fmax),
        "box in front must be visible"
    );

    let (bmin, bmax) = box_at(Vec3::new(0.0, 0.0, 5.0));
    assert!(
        !f.intersects_aabb(bmin, bmax),
        "box behind the camera must be culled"
    );

    let (side_min, side_max) = box_at(Vec3::new(100.0, 0.0, -5.0));
    assert!(
        !f.intersects_aabb(side_min, side_max),
        "box far off to the side must be culled"
    );
}

#[test]
fn a_caster_behind_the_camera_still_lies_inside_the_light_volume() {
    // The pop-a-shadow fix: a caster the *camera* frustum rejects (it is behind the
    // player) is kept by the *light* frustum, so it still throws a shadow into view.
    let camera = camera_frustum();
    let light = light_frustum(Vec3::new(-0.5, -1.0, -0.3));

    let (min, max) = box_at(Vec3::new(0.0, 0.0, 6.0)); // behind the camera, near origin
    assert!(
        !camera.intersects_aabb(min, max),
        "caster is behind the camera",
    );
    assert!(
        light.intersects_aabb(min, max),
        "caster is inside the light's ortho volume, so it must not be culled from shadows",
    );
}

#[test]
fn geometry_outside_the_light_volume_is_culled_from_shadows() {
    // Well outside the ±30 ortho box: nothing there can shadow the visible scene.
    let light = light_frustum(Vec3::new(-0.5, -1.0, -0.3));
    let (min, max) = box_at(Vec3::new(500.0, 0.0, 0.0));
    assert!(!light.intersects_aabb(min, max));
}

#[test]
fn transform_aabb_scales_and_translates_corners() {
    // Unit box, scaled ×2 then translated +10 on X.
    let world =
        Mat4::from_translation(Vec3::new(10.0, 0.0, 0.0)) * Mat4::from_scale(Vec3::splat(2.0));
    let (min, max) = transform_aabb(Vec3::splat(-1.0), Vec3::splat(1.0), world);
    assert_eq!(min, Vec3::new(8.0, -2.0, -2.0));
    assert_eq!(max, Vec3::new(12.0, 2.0, 2.0));
}

#[test]
fn transform_aabb_is_correct_under_rotation() {
    // Rotate a unit box 90° about Y: extents stay symmetric (still [-1,1] on X/Z).
    let world = Mat4::from_rotation_y(90_f32.to_radians());
    let (min, max) = transform_aabb(Vec3::splat(-1.0), Vec3::splat(1.0), world);
    // Allow for float error from the rotation.
    assert!((min - Vec3::splat(-1.0)).length() < 1e-5);
    assert!((max - Vec3::splat(1.0)).length() < 1e-5);
}
