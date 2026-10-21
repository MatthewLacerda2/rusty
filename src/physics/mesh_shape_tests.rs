//! src/physics/mesh_shape_tests.rs — which mesh colliders build, and which are skipped (#1132),
//! and a cylinder's radius under a stretched Z (#1131).

use glam::Vec3;
use rapier3d::prelude::*;

use super::build::build_shape;
use super::build_tests::mesh_shape;
use crate::components::ColliderShape;

const TRIANGLE: [[f32; 3]; 3] = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];

#[test]
fn a_three_vertex_trimesh_builds() {
    let mesh = Some((TRIANGLE.as_slice(), [0u32, 1, 2].as_slice()));
    let c = build_shape(&mesh_shape(false), Vec3::ONE, mesh).expect("one triangle is a trimesh");
    assert!(c.shape().as_trimesh().is_some());
}

#[test]
fn a_three_vertex_convex_hull_is_skipped() {
    let mesh = Some((TRIANGLE.as_slice(), [0u32, 1, 2].as_slice()));
    assert!(build_shape(&mesh_shape(true), Vec3::ONE, mesh).is_none());
}

#[test]
fn a_trimesh_without_a_triangle_is_skipped() {
    let mesh = Some((TRIANGLE.as_slice(), [0u32, 1].as_slice()));
    assert!(build_shape(&mesh_shape(false), Vec3::ONE, mesh).is_none());
}

#[test]
fn a_cylinder_ray_along_z_hits_the_stretched_radius() {
    let shape = ColliderShape::Cylinder {
        radius: 0.5,
        height: 2.0,
    };
    let c = build_shape(&shape, Vec3::new(1.0, 1.0, 3.0), None).unwrap();
    // Radius 0.5 * max(1, 3) = 1.5: a ray from z = -5 meets the surface at 3.5.
    let ray = Ray::new(Vector::new(0.0, 0.0, -5.0), Vector::new(0.0, 0.0, 1.0));
    let t = c
        .shape()
        .cast_ray(&Pose::identity(), &ray, 100.0, true)
        .expect("the ray hits");
    assert!((t - 3.5).abs() < 1e-3, "hit at {t}");
}
