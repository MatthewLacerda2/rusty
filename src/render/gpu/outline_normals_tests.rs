//! The selection outline's normals (#1175): a hard-edged mesh's faces push out
//! together, and a smooth mesh keeps its own. No GPU.

use glam::Vec3;

use super::mesh::outline_normals;
use crate::components::mesh::primitives::{generate_box, generate_sphere};

#[test]
fn a_box_corner_grows_along_its_diagonal() {
    let (vertices, _) = generate_box(1.0, 2.0, 1.0);
    for (v, n) in vertices.iter().zip(outline_normals(&vertices)) {
        let diagonal = Vec3::from(v.position).signum().normalize();
        assert!(
            Vec3::from(n).abs_diff_eq(diagonal, 1e-5),
            "{:?}: {n:?}, want {diagonal}",
            v.position
        );
    }
}

#[test]
fn a_smooth_mesh_keeps_its_normals() {
    let (vertices, _) = generate_sphere(0.8, 12, 16);
    for (v, n) in vertices.iter().zip(outline_normals(&vertices)) {
        let own = Vec3::from(v.normal).normalize();
        assert!(Vec3::from(n).abs_diff_eq(own, 1e-4), "{:?}", v.position);
    }
}
