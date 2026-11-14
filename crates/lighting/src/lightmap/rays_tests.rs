//! The bake's ray queries (#1330): a hit names the triangle it landed on and where
//! on it, from either side, and the shadow query sees exactly what lies in reach.

use glam::Vec3;

use super::rays::{Rays, Tri};

/// A 2×2 quad at z = 0 (triangles 0 and 1, facing +Z) and a triangle at z = -1
/// behind it (triangle 2), each tagged with its own `first`.
fn scene() -> Rays {
    let (a, b, c, d) = (
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(2.0, 0.0, 0.0),
        Vec3::new(2.0, 2.0, 0.0),
        Vec3::new(0.0, 2.0, 0.0),
    );
    let back = |v: Vec3| v - Vec3::Z;
    Rays::build(vec![
        Tri::new(a, b, c, 0, 0),
        Tri::new(a, c, d, 0, 3),
        Tri::new(back(a), back(b), back(d), 1, 0),
    ])
}

/// The world point a hit's barycentrics name.
fn point(rays: &Rays, tri: usize, u: f32, v: f32) -> Vec3 {
    let t = &rays.tris[tri];
    t.a + t.e1 * u + t.e2 * v
}

#[test]
fn a_hit_reports_its_triangle_and_barycentrics() {
    let rays = scene();
    // (1.5, 0.5) lies in triangle 0 (a, b, c): a + u·(b − a) + v·(c − a) there
    // gives u = 0.5, v = 0.25.
    let hit = rays
        .closest(Vec3::new(1.5, 0.5, 3.0), Vec3::NEG_Z, 1e-4, f32::MAX)
        .expect("the quad is in the way");
    assert_eq!((hit.tri, rays.tris[hit.tri].first), (0, 0));
    assert!(
        (hit.u - 0.5).abs() < 1e-5 && (hit.v - 0.25).abs() < 1e-5,
        "{hit:?}"
    );
    // (0.5, 1.5) lies in triangle 1 (a, c, d): u = 0.25, v = 0.5, and they name
    // the point the ray crossed.
    let hit = rays
        .closest(Vec3::new(0.5, 1.5, 3.0), Vec3::NEG_Z, 1e-4, f32::MAX)
        .expect("the quad is in the way");
    assert_eq!((hit.tri, rays.tris[hit.tri].first), (1, 3));
    assert!(
        (hit.u - 0.25).abs() < 1e-5 && (hit.v - 0.5).abs() < 1e-5,
        "{hit:?}"
    );
    let at = point(&rays, hit.tri, hit.u, hit.v);
    assert!(at.distance(Vec3::new(0.5, 1.5, 0.0)) < 1e-5, "{at}");
}

#[test]
fn the_nearest_face_wins_from_either_side() {
    let rays = scene();
    // From behind, the triangle at z = -1 is nearer than the quad, and its back
    // face is still a hit: the tracer, not the query, decides what a back face means.
    let hit = rays
        .closest(Vec3::new(0.5, 0.5, -3.0), Vec3::Z, 1e-4, f32::MAX)
        .expect("the back triangle is in the way");
    assert_eq!(hit.tri, 2);
    assert!(
        rays.tris[hit.tri].normal().dot(Vec3::Z) > 0.0,
        "a back-face hit"
    );
    let at = point(&rays, hit.tri, hit.u, hit.v);
    assert!(at.distance(Vec3::new(0.5, 0.5, -1.0)) < 1e-5, "{at}");
}

#[test]
fn hits_respect_the_ray_s_reach() {
    let rays = scene();
    let (origin, down) = (Vec3::new(0.5, 0.5, 3.0), Vec3::NEG_Z);
    // Starting past the quad (t_min beyond it) the triangle behind is the first hit.
    let hit = rays
        .closest(origin, down, 3.5, f32::MAX)
        .expect("triangle 2");
    assert_eq!(hit.tri, 2);
    // Stopping short of the quad, nothing is hit and nothing blocks.
    assert!(rays.closest(origin, down, 1e-4, 2.5).is_none());
    assert!(!rays.occluded(origin, down, 1e-4, 2.5));
    assert!(rays.occluded(origin, down, 1e-4, 3.5));
    // The reach ends before `t_max`: the quad at exactly t = 3 is out of it for
    // both queries, and in it just past.
    assert!(rays.closest(origin, down, 0.0, 3.0).is_none());
    assert!(!rays.occluded(origin, down, 0.0, 3.0));
    assert!(rays.closest(origin, down, 0.0, 3.01).is_some());
    assert!(rays.occluded(origin, down, 0.0, 3.01));
    // A ray that misses everything.
    assert!(!rays.occluded(Vec3::new(5.0, 5.0, 3.0), down, 1e-4, f32::MAX));
}

#[test]
fn an_empty_scene_hits_nothing() {
    let rays = Rays::build(Vec::new());
    assert!(rays.closest(Vec3::ZERO, Vec3::X, 1e-4, f32::MAX).is_none());
    assert!(!rays.occluded(Vec3::ZERO, Vec3::X, 1e-4, f32::MAX));
}
