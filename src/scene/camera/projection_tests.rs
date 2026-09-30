use glam::{Vec2, Vec3};

use crate::scene::Camera;

const SCREEN: Vec2 = Vec2::new(1600.0, 900.0);

/// A camera at the origin looking down -Z (yaw -90°), 90° vertical fov.
fn camera() -> Camera {
    let mut cam = Camera::new(Vec3::ZERO, -90.0, 0.0);
    cam.fov = 90.0;
    cam
}

fn close(a: Vec2, b: Vec2) -> bool {
    (a - b).abs().max_element() < 1e-2
}

#[test]
fn a_point_ahead_projects_to_the_centre() {
    let p = camera().world_to_screen(Vec3::new(0.0, 0.0, -5.0), SCREEN);
    assert!(close(p.position, SCREEN * 0.5));
    assert!((p.depth - 5.0).abs() < 1e-4);
    assert!(p.is_on_screen(SCREEN));
}

#[test]
fn up_and_right_follow_the_screen_frame() {
    // 90° fov: at depth 2 the half-height is 2, so y = 1 is a quarter up from centre.
    let p = camera().world_to_screen(Vec3::new(0.0, 1.0, -2.0), SCREEN);
    assert!(close(p.position, Vec2::new(800.0, 450.0 + 225.0)));
    // Half-width is 2 · aspect; x = 2 · aspect lands on the right edge.
    let edge = 2.0 * SCREEN.x / SCREEN.y;
    let p = camera().world_to_screen(Vec3::new(edge, 0.0, -2.0), SCREEN);
    assert!(close(p.position, Vec2::new(1600.0, 450.0)));
}

#[test]
fn a_point_behind_is_flagged_and_mirrored() {
    let p = camera().world_to_screen(Vec3::new(1.0, 0.0, 5.0), SCREEN);
    assert!(p.is_behind());
    assert!(!p.is_on_screen(SCREEN));
    // Right of the camera but behind it lands left of centre.
    assert!(p.position.x < 800.0);
}

#[test]
fn screen_ray_inverts_world_to_screen() {
    let cam = camera();
    let target = Vec3::new(1.5, -0.7, -6.0);
    let px = cam.world_to_screen(target, SCREEN).position;
    let (origin, dir) = cam.screen_ray(px, SCREEN);
    assert!((dir.length() - 1.0).abs() < 1e-5);
    // The target lies on the ray.
    let along = (target - origin).dot(dir);
    assert!((origin + dir * along - target).length() < 1e-3);
    // The centre ray is the view direction.
    let (_, centre) = cam.screen_ray(SCREEN * 0.5, SCREEN);
    assert!((centre - cam.forward()).length() < 1e-5);
}
