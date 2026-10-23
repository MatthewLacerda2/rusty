//! src/physics/shape/fixture.rs — the shape-level test meshes, shared by the
//! conformance suite, the hull tests and the point-query tests.

use glam::Vec3;

use crate::components::mesh::Vertex;
use crate::scene::authoring::{primitive_geometry, Primitive};

/// The Cylinder primitive moved off its origin, so its local box is not centred:
/// rings of coplanar points, the classic quickhull degeneracy.
pub(in crate::physics) fn off_centre_cylinder() -> (Vec<Vertex>, Vec<u32>) {
    let (mut vertices, indices) = primitive_geometry(Primitive::Cylinder).expect("a mesh");
    let offset = Vec3::new(0.3, 0.2, -0.1);
    vertices
        .iter_mut()
        .for_each(|v| v.position = (Vec3::from(v.position) + offset).to_array());
    (vertices, indices)
}
