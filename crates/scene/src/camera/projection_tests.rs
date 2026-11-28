use glam::{Vec2, Vec3};

use crate::Camera;

const SCREEN: Vec2 = Vec2::new(1600.0, 900.0);

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
    // 90° fov: at depth 2 the half-height is 2, so y = 1 lands a quarter up; x = 2 · aspect
    // lands on the right edge.
    let p = camera().world_to_screen(Vec3::new(0.0, 1.0, -2.0), SCREEN);
    assert!(close(p.position, Vec2::new(800.0, 450.0 + 225.0)));
    let edge = 2.0 * SCREEN.x / SCREEN.y;
    let p = camera().world_to_screen(Vec3::new(edge, 0.0, -2.0), SCREEN);
    assert!(close(p.position, Vec2::new(1600.0, 450.0)));
}

#[test]
fn a_point_behind_is_flagged_and_mirrored() {
    let p = camera().world_to_screen(Vec3::new(1.0, 0.0, 5.0), SCREEN);
    assert!(p.is_behind());
    assert!(!p.is_on_screen(SCREEN));
    assert!(p.position.x < 800.0);
}

#[test]
fn screen_ray_inverts_world_to_screen() {
    let cam = camera();
    let target = Vec3::new(1.5, -0.7, -6.0);
    let px = cam.world_to_screen(target, SCREEN).position;
    let (origin, dir) = cam.screen_ray(px, SCREEN);
    assert!((dir.length() - 1.0).abs() < 1e-5);
    let along = (target - origin).dot(dir);
    assert!((origin + dir * along - target).length() < 1e-3);
    let (_, centre) = cam.screen_ray(SCREEN * 0.5, SCREEN);
    assert!((centre - cam.forward()).length() < 1e-5);
}

#[test]
fn orthographic_maps_its_half_height_to_the_screen_edge() {
    use rusty_components::Projection;
    let mut cam = camera();
    cam.projection = Projection::Orthographic { size: 10.0 };
    let screen = Vec2::new(200.0, 100.0);
    let ahead = cam.position + cam.forward() * 50.0;
    let up = cam.world_to_screen(ahead + Vec3::Y * 10.0, screen);
    assert!((up.position.y - 100.0).abs() < 1e-3, "{up:?}");
    assert!((up.depth - 50.0).abs() < 1e-3, "depth is the view distance");
    let (_, dir) = cam.screen_ray(Vec2::new(10.0, 90.0), screen);
    assert!(dir.dot(cam.forward()) > 0.9999);
}

#[test]
fn screen_ray_round_trips_points_on_the_near_and_far_planes() {
    let mut cam = camera();
    cam.near = 0.1;
    cam.far = 50_000.0;
    let (origin, _) = cam.screen_ray(SCREEN * 0.5, SCREEN);
    assert!(
        ((origin - cam.position).dot(cam.forward()) - 0.1).abs() < 1e-4,
        "{origin}"
    );
    for (at, tolerance) in [(0.1, 1e-4), (50_000.0, 1.0)] {
        // `at` along the forward axis: on the plane itself.
        let target = cam.position + Vec3::new(0.3, -0.2, -1.0) * at;
        let px = cam.world_to_screen(target, SCREEN).position;
        let (origin, dir) = cam.screen_ray(px, SCREEN);
        let along = (target - origin).dot(dir);
        assert!(along >= -1e-4, "the ray runs toward the point at {at} m");
        assert!(
            (origin + dir * along - target).length() < tolerance,
            "at {at} m"
        );
    }
}
