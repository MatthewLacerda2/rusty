//! src/scene/authoring/rendering/mesh.rs — the Mesh authoring op (#1524).
//!
//! A mesh's geometry comes from the model or primitive it was made from; the one
//! value authored on it is a `Plane`'s subdivisions, the grid a vertex block
//! (`vertex_waves`) needs to move. The Inspector's Mesh card and `Component.Set` both
//! write it here.
//!
//! Pure.

use crate::components::mesh::primitives::MAX_PLANE_SUBDIVISIONS;
use crate::components::MeshComponent;
use crate::scene::authoring::{subdivided_geometry, Primitive};

/// Cut a `Plane` into `subdivisions` quads per side and rebuild its geometry.
/// Refuses a count outside `1..=`[`MAX_PLANE_SUBDIVISIONS`], and any count but 1 on
/// a mesh that isn't a `Plane`.
pub fn set_subdivisions(mesh: &mut MeshComponent, subdivisions: u32) -> Result<(), String> {
    if !(1..=MAX_PLANE_SUBDIVISIONS).contains(&subdivisions) {
        return Err(format!(
            "subdivisions must be 1..={MAX_PLANE_SUBDIVISIONS}, got {subdivisions}"
        ));
    }
    if subdivisions == mesh.subdivisions {
        return Ok(());
    }
    if mesh.primitive_type != "Plane" {
        return Err(format!(
            "subdivisions shapes a Plane; a {} mesh keeps 1",
            mesh.primitive_type
        ));
    }
    let (vertices, indices) =
        subdivided_geometry(Primitive::Plane, subdivisions).expect("a Plane has geometry");
    mesh.subdivisions = subdivisions;
    mesh.vertices = vertices;
    mesh.indices = indices;
    mesh.is_dirty.set(true);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::authoring::primitive_mesh_component;

    #[test]
    fn a_subdivided_plane_has_a_vertex_per_grid_corner() {
        let mut plane = primitive_mesh_component(Primitive::Plane).unwrap();
        assert_eq!(plane.vertices.len(), 4);
        set_subdivisions(&mut plane, 64).unwrap();
        assert_eq!(plane.vertices.len(), 65 * 65);
        assert_eq!(plane.indices.len(), 64 * 64 * 6);
        assert!(plane.is_dirty.get());
        set_subdivisions(&mut plane, 1).unwrap();
        assert_eq!(plane.vertices.len(), 4, "1 is the single quad again");
    }

    #[test]
    fn out_of_range_counts_and_other_shapes_are_refused() {
        let mut plane = primitive_mesh_component(Primitive::Plane).unwrap();
        for bad in [0, MAX_PLANE_SUBDIVISIONS + 1] {
            let e = set_subdivisions(&mut plane, bad).unwrap_err();
            assert!(e.contains("1..=256"), "{e}");
        }
        assert_eq!(plane.subdivisions, 1);
        let mut cube = primitive_mesh_component(Primitive::Box).unwrap();
        assert!(set_subdivisions(&mut cube, 4)
            .unwrap_err()
            .contains("Plane"));
        assert_eq!(set_subdivisions(&mut cube, 1), Ok(()));
    }
}
