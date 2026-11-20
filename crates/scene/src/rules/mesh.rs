//! crates/scene/src/rules/mesh.rs — the Mesh fields a file can carry out of range
//! (#1524, #1704), held where rehydration rebuilds the mesh.
//!
//! The field table leaves these fields `unheld!` (a load's generic hold would copy
//! the mesh's whole geometry to hold one number), so rehydration runs these on
//! every scene load and prefab instantiation. The Mesh ops
//! (`authoring::rendering::mesh`) hold the same bounds on a write; the subdivision
//! op itself is [`set_subdivisions`], here so a renderer test can cut a Plane
//! without the authoring layer.
//!
//! Pure.

use super::sanitize;
use crate::create::{primitive_revision, subdivided_geometry, Primitive};
use rusty_components::mesh::primitives::MAX_PLANE_SUBDIVISIONS;
use rusty_components::MeshComponent;

/// Hold a Scale In Lightmap read from a file (#1704) in
/// [`MeshComponent::SCALE_IN_LIGHTMAP`] (a non-finite one reads 1). Returns the
/// warning when it moved one. The field table leaves the field `unheld!` for
/// this, as for [`hold_subdivisions`]: a load's generic hold would copy the
/// mesh's whole geometry to hold one number.
pub fn hold_scale_in_lightmap(mesh: &mut MeshComponent) -> Option<String> {
    let read = mesh.scale_in_lightmap;
    let (lo, hi) = MeshComponent::SCALE_IN_LIGHTMAP.into_inner();
    let held = sanitize::finite_or(read, 1.0).clamp(lo, hi);
    (held.to_bits() != read.to_bits()).then(|| {
        mesh.scale_in_lightmap = held;
        format!("a mesh's scale_in_lightmap {read} was held at {held} ({lo}..={hi})")
    })
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

/// Cut a `Plane` into `subdivisions` quads per side and rebuild its geometry.
/// Refuses a count outside `1..=`[`MAX_PLANE_SUBDIVISIONS`], and any count but 1 on
/// a mesh that isn't a `Plane`. The Mesh op (`authoring::rendering::mesh`) is this.
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
    mesh.vertices = vertices.into();
    mesh.indices = indices.into();
    mesh.revision = primitive_revision(Primitive::Plane, subdivisions);
    mesh.own_geometry = None;
    Ok(())
}
