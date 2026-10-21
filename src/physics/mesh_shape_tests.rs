//! src/physics/mesh_shape_tests.rs — which mesh colliders build, and which are skipped (#1132),
//! and a cylinder's radius under a stretched Z (#1131).

use glam::Vec3;
use rapier3d::prelude::*;

use super::build::{build_shape, try_build_shape};
use super::build_tests::mesh_shape;
use super::mesh_shape::Skip;
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

#[test]
fn an_index_past_the_vertices_is_skipped() {
    let mesh = Some((TRIANGLE.as_slice(), [0u32, 1, 3].as_slice()));
    assert!(build_shape(&mesh_shape(false), Vec3::ONE, mesh).is_none());
}

#[test]
fn a_trimesh_of_collinear_points_is_skipped() {
    let line = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0]];
    let mesh = Some((line.as_slice(), [0u32, 1, 2].as_slice()));
    assert!(build_shape(&mesh_shape(false), Vec3::ONE, mesh).is_none());
}

#[test]
fn a_ramp_scaled_flat_is_skipped_but_one_good_triangle_keeps_the_mesh() {
    let mesh = Some((TRIANGLE.as_slice(), [0u32, 1, 2].as_slice()));
    assert!(build_shape(&mesh_shape(false), Vec3::new(1.0, 0.0, 1.0), mesh).is_none());
    let quad = [
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [2.0, 0.0, 0.0],
    ];
    let mixed = Some((quad.as_slice(), [0u32, 1, 3, 0, 1, 2].as_slice()));
    assert!(build_shape(&mesh_shape(false), Vec3::ONE, mixed).is_some());
}

/// Parry takes coplanar points as a flat hull; collinear ones are what it refuses.
#[test]
fn four_collinear_points_form_no_convex_hull() {
    let line = [
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [2.0, 0.0, 0.0],
        [3.0, 0.0, 0.0],
    ];
    let mesh = Some((line.as_slice(), [0u32, 1, 2].as_slice()));
    assert!(build_shape(&mesh_shape(true), Vec3::ONE, mesh).is_none());
}

#[test]
fn a_small_tile_far_from_the_origin_still_builds() {
    let tile = [[1500.0, 0.0, 0.0], [1501.0, 0.0, 0.0], [1500.0, 0.0, 1.0]];
    let mesh = Some((tile.as_slice(), [0u32, 1, 2].as_slice()));
    assert!(build_shape(&mesh_shape(false), Vec3::ONE, mesh).is_some());
}

#[test]
fn a_convex_hull_ignores_a_stale_index() {
    let tetra = [
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
    ];
    let mesh = Some((tetra.as_slice(), [0u32, 1, 99].as_slice()));
    assert!(build_shape(&mesh_shape(true), Vec3::ONE, mesh).is_some());
}

#[test]
fn each_skip_names_its_reason() {
    let shape = mesh_shape(false);
    let skip = |idx: &[u32], mesh: bool| {
        let m = mesh.then_some((TRIANGLE.as_slice(), idx));
        try_build_shape(&shape, Vec3::ONE, m).err()
    };
    assert_eq!(skip(&[0, 1, 2], false), Some(Skip::NoMesh));
    assert_eq!(
        skip(&[0, 1, 3], true),
        Some(Skip::IndexOutOfRange {
            index: 3,
            vertices: 3
        })
    );
    assert_eq!(skip(&[0, 1], true), Some(Skip::NoTriangles));
}
