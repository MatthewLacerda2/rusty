//! src/scene/authoring/rendering/mesh.rs — the Mesh authoring op (#1524).
//!
//! A mesh's geometry comes from the model or primitive it was made from; the values
//! authored on it are a `Plane`'s subdivisions, the grid a vertex block
//! (`vertex_waves`) needs to move, and how it casts shadows (#1571). The Inspector's
//! Mesh card and `Component.Set` both write them here.
//!
//! Pure.

use crate::components::mesh::primitives::MAX_PLANE_SUBDIVISIONS;
use crate::components::{MeshComponent, ShadowCasting};
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
    if mesh.primitive_type != "Plane" && subdivisions == 1 {
        mesh.subdivisions = 1; // clears a stray count; the shape ignores it anyway
        return Ok(());
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
    mesh.revision = crate::components::MeshRevision::next();
    Ok(())
}

/// Set how the mesh casts shadows (#1571): every shadow path reads it on its next
/// frame or bake, a static one's cached shadow included.
pub fn set_cast_shadows(mesh: &mut MeshComponent, mode: ShadowCasting) {
    mesh.cast_shadows = mode;
}

/// Hold a count read from a file (#1524), so the field agrees with the grid the
/// rehydrated mesh is built from: out of range clamps into `1..=`
/// [`MAX_PLANE_SUBDIVISIONS`], and a count on a mesh that isn't a `Plane` resets to
/// one quad. Returns the warning when it moved one, `None` when it was already valid.
/// The field table leaves the field `unheld!` for this: rehydration runs it on every
/// scene load and prefab instantiation, before the World sees the mesh.
pub fn hold_subdivisions(mesh: &mut MeshComponent) -> Option<String> {
    let read = mesh.subdivisions;
    let held = match mesh.primitive_type.as_str() {
        "Plane" => read.clamp(1, MAX_PLANE_SUBDIVISIONS),
        _ => 1,
    };
    (held != read).then(|| {
        mesh.subdivisions = held;
        format!(
            "a {} mesh's subdivisions {read} was held at {held} (1..={MAX_PLANE_SUBDIVISIONS}, Plane only)",
            mesh.primitive_type
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::authoring::primitive_mesh_component;

    #[test]
    fn a_subdivided_plane_has_a_vertex_per_grid_corner() {
        let mut plane = primitive_mesh_component(Primitive::Plane).unwrap();
        assert_eq!(plane.vertices.len(), 4);
        let before = plane.revision;
        set_subdivisions(&mut plane, 64).unwrap();
        assert_eq!(plane.vertices.len(), 65 * 65);
        assert_eq!(plane.indices.len(), 64 * 64 * 6);
        assert!(
            plane.revision > before,
            "a rebuilt grid takes a fresh revision"
        );
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
        cube.subdivisions = 4; // a stray count, from a hand-edited file
        assert_eq!(set_subdivisions(&mut cube, 1), Ok(()), "1 clears it");
        assert_eq!(cube.subdivisions, 1);
    }

    #[test]
    fn a_loaded_count_is_held_into_what_the_grid_builds() {
        let mut plane = primitive_mesh_component(Primitive::Plane).unwrap();
        for (read, held) in [(0, Some(1)), (500, Some(256)), (64, None)] {
            plane.subdivisions = read;
            let warned = hold_subdivisions(&mut plane);
            assert_eq!(warned.is_some(), held.is_some(), "{read}: {warned:?}");
            assert_eq!(plane.subdivisions, held.unwrap_or(read));
        }
        let mut cube = primitive_mesh_component(Primitive::Box).unwrap();
        cube.subdivisions = 4;
        assert!(hold_subdivisions(&mut cube).unwrap().contains("Box"));
        assert_eq!(cube.subdivisions, 1);
    }
}
