use glam::{Vec3, Vec4};

use super::{in_front, orthographic, perspective, FAR, NEAR};

fn depth_at(proj: glam::Mat4, distance: f32) -> f32 {
    let clip = proj * Vec4::new(0.0, 0.0, -distance, 1.0);
    clip.z / clip.w
}

#[test]
fn the_near_plane_is_one_and_the_far_plane_zero() {
    for proj in [
        perspective(1.0, 1.5, 0.1, 200.0),
        orthographic(-2.0, 2.0, -1.0, 1.0, 0.1, 200.0),
    ] {
        assert!((depth_at(proj, 0.1) - NEAR).abs() < 1e-6);
        assert!((depth_at(proj, 200.0) - FAR).abs() < 1e-6);
        assert!(in_front(depth_at(proj, 5.0), depth_at(proj, 6.0)));
    }
}

#[test]
fn a_centimetre_at_twenty_kilometres_keeps_its_order() {
    // The precision reversed depth exists for: near 0.1, far 50 km, two surfaces 1 cm apart.
    let proj = perspective(1.0, 1.0, 0.1, 50_000.0);
    for at in [5_000.0, 20_000.0] {
        assert!(
            in_front(depth_at(proj, at), depth_at(proj, at + 0.01)),
            "at {at} m"
        );
    }
    let unprojected = proj
        .inverse()
        .project_point3(Vec3::new(0.0, 0.0, depth_at(proj, 20_000.0)));
    assert!((unprojected.z + 20_000.0).abs() < 1.0, "{unprojected}");
}
