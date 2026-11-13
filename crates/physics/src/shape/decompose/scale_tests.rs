//! crates/physics/src/shape/decompose/scale_tests.rs — one decomposition per mesh, at unit
//! scale; a body's scale is applied to the hulls (#1940).

use std::sync::Arc;

use glam::Vec3;
use rapier3d::prelude::*;

use super::RUNS;
use crate::shape::build::try_build_body_shape;
use crate::shape::build_tests::mesh_shape;
use crate::shape::fixture::arch;

type Mesh = (Vec<[f32; 3]>, Vec<u32>);

/// The arch moved by `x`: a mesh no other test decomposes, so its first build here
/// is a run whatever ran before in this process.
fn own_arch(x: f32) -> Mesh {
    let (mut p, i) = arch();
    p.iter_mut().for_each(|p| p[0] += x);
    (p, i)
}

fn build(mesh: &Mesh, scale: Vec3) -> Collider {
    let geom = Some((mesh.0.as_slice(), mesh.1.as_slice()));
    try_build_body_shape(&mesh_shape(false), scale, geom, true).expect("builds")
}

fn runs() -> usize {
    RUNS.with(|r| r.get())
}

fn volume(c: &Collider) -> f32 {
    c.shape().mass_properties(1.0).mass()
}

#[test]
fn copies_share_a_shape_and_another_scale_runs_no_vhacd() {
    let mesh = own_arch(19.401);
    let before = runs();
    let first = build(&mesh, Vec3::ONE);
    assert_eq!(runs(), before + 1, "the first build decomposes");
    let second = build(&mesh, Vec3::ONE);
    let same = |a: &Collider, b: &Collider| Arc::ptr_eq(&a.shared_shape().0, &b.shared_shape().0);
    assert!(same(&first, &second), "a copy is a cache hit");
    let stretched = build(&mesh, Vec3::new(1.5, 2.0, 0.5));
    let mirrored = build(&mesh, Vec3::new(-1.0, 1.0, 1.0));
    assert_eq!(runs(), before + 1, "a new scale scales the hulls");
    assert!(!same(&first, &stretched));
    assert!(stretched.shape().as_compound().is_some());
    let pieces = |c: &Collider| c.shape().as_compound().unwrap().shapes().len();
    assert_eq!(pieces(&stretched), pieces(&first));
    assert_eq!(pieces(&mirrored), pieces(&first));
    // A linear map scales a volume by its determinant's magnitude.
    let want = volume(&first) * 1.5;
    assert!((volume(&stretched) - want).abs() < want * 1e-3, "stretched");
    let want = volume(&first);
    assert!((volume(&mirrored) - want).abs() < want * 1e-3, "mirrored");
}

#[test]
fn a_mirrored_copy_is_the_mirror_image_with_its_faces_out() {
    let mesh = own_arch(19.402);
    let unit = build(&mesh, Vec3::ONE).compute_aabb();
    let mirrored = build(&mesh, Vec3::new(-1.0, 1.0, 1.0));
    let aabb = mirrored.compute_aabb();
    assert!(
        (aabb.mins.x + unit.maxs.x).abs() < 1e-4,
        "{aabb:?} vs {unit:?}"
    );
    assert!(
        (aabb.maxs.x + unit.mins.x).abs() < 1e-4,
        "{aabb:?} vs {unit:?}"
    );
    // Faces out: a ray from outside hits the mirrored solid's near side.
    let ray = Ray::new(Vector::new(-19.4, 0.8, -5.0), Vector::Z);
    let hit = mirrored.shape().cast_ray(&Pose::IDENTITY, &ray, 10.0, true);
    assert!(hit.is_some_and(|t| t > 4.0 && t < 5.0), "hit at {hit:?}");
}
