use super::{transform_aabb, Frustum};
use glam::camera::rh::{proj::directx, view::look_at_mat4};
use glam::{Mat4, Vec3};

fn camera_frustum() -> Frustum {
    let view = look_at_mat4(Vec3::ZERO, Vec3::new(0.0, 0.0, -1.0), Vec3::Y);
    let proj = directx::perspective(60_f32.to_radians(), 1.0, 0.1, 100.0);
    Frustum::from_view_proj(proj * view)
}

fn light_frustum(light_dir: Vec3) -> Frustum {
    let dir = light_dir.normalize();
    let view = look_at_mat4(-dir * 45.0, Vec3::ZERO, Vec3::Y);
    let proj = directx::orthographic(-30.0, 30.0, -30.0, 30.0, 1.0, 100.0);
    Frustum::from_view_proj(proj * view)
}

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
    let camera = camera_frustum();
    let light = light_frustum(Vec3::new(-0.5, -1.0, -0.3));

    let (min, max) = box_at(Vec3::new(0.0, 0.0, 6.0));
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
    let light = light_frustum(Vec3::new(-0.5, -1.0, -0.3));
    let (min, max) = box_at(Vec3::new(500.0, 0.0, 0.0));
    assert!(!light.intersects_aabb(min, max));
}

#[test]
fn transform_aabb_scales_and_translates_corners() {
    let world =
        Mat4::from_translation(Vec3::new(10.0, 0.0, 0.0)) * Mat4::from_scale(Vec3::splat(2.0));
    let (min, max) = transform_aabb(Vec3::splat(-1.0), Vec3::splat(1.0), world);
    assert_eq!(min, Vec3::new(8.0, -2.0, -2.0));
    assert_eq!(max, Vec3::new(12.0, 2.0, 2.0));
}

#[test]
fn transform_aabb_is_correct_under_rotation() {
    let world = Mat4::from_rotation_y(90_f32.to_radians());
    let (min, max) = transform_aabb(Vec3::splat(-1.0), Vec3::splat(1.0), world);
    assert!((min - Vec3::splat(-1.0)).length() < 1e-5);
    assert!((max - Vec3::splat(1.0)).length() < 1e-5);
}
