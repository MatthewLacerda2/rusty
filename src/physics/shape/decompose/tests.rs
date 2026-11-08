//! src/physics/shape/decompose/tests.rs — a concave mesh on a dynamic body becomes
//! convex pieces, the same ones every run, made once per mesh (#1886).

use std::cell::RefCell;

use glam::Vec3;
use rapier3d::prelude::*;

use super::{assemble, decompose, decomposition};
use crate::physics::shape::build::{try_build_body_shape, try_build_shape};
use crate::physics::shape::build_tests::mesh_shape;
use crate::physics::shape::fit::build_shape_for;
use crate::physics::shape::fixture::{arch, bowl};
use crate::scripting::ConsoleLogs;

fn build(mesh: &(Vec<[f32; 3]>, Vec<u32>), scale: Vec3, dynamic: bool) -> Collider {
    let mesh = Some((mesh.0.as_slice(), mesh.1.as_slice()));
    try_build_body_shape(&mesh_shape(false), scale, mesh, dynamic).expect("builds")
}

pub(super) fn points(mesh: &(Vec<[f32; 3]>, Vec<u32>)) -> (Vec<Vector>, Vec<[u32; 3]>) {
    let p = mesh.0.iter().map(|&p| Vector::from_array(p)).collect();
    let t = mesh.1.chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect();
    (p, t)
}

/// Every hull's points, in order: the decomposition's bits.
pub(super) fn hull_bits(shape: &SharedShape) -> Vec<[u32; 3]> {
    let compound = shape.as_compound().expect("a compound");
    (compound.shapes().iter())
        .flat_map(|(_, s)| s.as_convex_polyhedron().expect("a hull").points().to_vec())
        .map(|p| p.to_array().map(f32::to_bits))
        .collect()
}

#[test]
fn a_concave_mesh_on_a_dynamic_body_is_convex_pieces_with_its_volume() {
    let c = build(&arch(), Vec3::ONE, true);
    let compound = c.shape().as_compound().expect("decomposed, not a trimesh");
    assert!(
        compound.shapes().len() >= 2,
        "an arch needs more than one hull"
    );
    let volume = c.shape().mass_properties(1.0).mass();
    assert!(
        (volume - 0.64).abs() < 0.64 * 0.2,
        "volume {volume}, want ≈ 0.64"
    );
    // The hollow under the arch stays open: one hull would fill it.
    let ray = Ray::new(Vector::new(0.0, 0.3, -5.0), Vector::Z);
    assert!(c
        .shape()
        .cast_ray(&Pose::IDENTITY, &ray, 10.0, true)
        .is_none());
}

#[test]
fn static_kinematic_and_convex_keep_their_shapes() {
    let mesh = arch();
    assert!(build(&mesh, Vec3::ONE, false)
        .shape()
        .as_trimesh()
        .is_some());
    let geom = Some((mesh.0.as_slice(), mesh.1.as_slice()));
    let hull = try_build_body_shape(&mesh_shape(true), Vec3::ONE, geom, true).unwrap();
    assert!(hull.shape().as_convex_polyhedron().is_some());
    let exact = try_build_shape(&mesh_shape(false), Vec3::ONE, geom).unwrap();
    assert!(exact.shape().as_trimesh().is_some());
}

#[test]
fn the_same_mesh_decomposes_to_the_same_bits() {
    let (p, t) = points(&bowl());
    let a = assemble(&decompose(&p, &t), Vec3::ONE).expect("pieces");
    let b = assemble(&decompose(&p, &t), Vec3::ONE).expect("pieces");
    assert_eq!(hull_bits(&a), hull_bits(&b));
}

#[test]
fn a_flat_concave_mesh_falls_back_to_the_trimesh() {
    // An L of two triangles' worth of floor: area, but no volume to split.
    #[rustfmt::skip]
    let flat = (
        vec![[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [2.0, 0.0, 1.0], [1.0, 0.0, 1.0],
             [1.0, 0.0, 2.0], [0.0, 0.0, 2.0]],
        vec![0, 2, 1, 0, 3, 2, 0, 4, 3, 0, 5, 4],
    );
    assert!(decomposition(&flat.0, &flat.1, Vec3::ONE).is_none());
    assert!(build(&flat, Vec3::ONE, true).shape().as_trimesh().is_some());
    // One warning, naming the entity; never a body without a collider.
    crate::logging::init();
    let console = RefCell::new(ConsoleLogs::new());
    crate::logging::forward(&console);
    let geom = Some((flat.0.as_slice(), flat.1.as_slice()));
    let built = build_shape_for(9200, &mesh_shape(false), Vec3::ONE, geom, true);
    assert!(built.is_some_and(|c| c.shape().as_trimesh().is_some()));
    let logged = crate::logging::forward(&console);
    let named = logged.iter().filter(|m| m.contains("entity 9200:")).count();
    assert_eq!(named, 1, "{logged:?}");
}

#[test]
fn a_non_finite_vertex_is_skipped_before_vhacd() {
    let mut mesh = arch();
    mesh.0[3][1] = f32::NAN;
    let geom = Some((mesh.0.as_slice(), mesh.1.as_slice()));
    let skip = try_build_body_shape(&mesh_shape(false), Vec3::ONE, geom, true).unwrap_err();
    assert_eq!(
        skip,
        crate::physics::shape::mesh::Skip::NonFinite { vertex: 3 }
    );
}
