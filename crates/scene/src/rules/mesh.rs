//! The Mesh fields a file can carry out of range, held on every load and prefab stamp: the
//! field table leaves them `unheld!`, since its hold would copy the whole geometry for one.

use super::sanitize;
use crate::create::{primitive_revision, subdivided_geometry, Primitive};
use rusty_components::mesh::primitives::MAX_PLANE_SUBDIVISIONS;
use rusty_components::MeshComponent;

pub fn hold_scale_in_lightmap(mesh: &mut MeshComponent) -> Option<String> {
    let read = mesh.scale_in_lightmap;
    let (lo, hi) = MeshComponent::SCALE_IN_LIGHTMAP.into_inner();
    let held = sanitize::finite_or(read, 1.0).clamp(lo, hi);
    (held.to_bits() != read.to_bits()).then(|| {
        mesh.scale_in_lightmap = held;
        format!("a mesh's scale_in_lightmap {read} was held at {held} ({lo}..={hi})")
    })
}

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

pub fn set_subdivisions(mesh: &mut MeshComponent, subdivisions: u32) -> Result<(), String> {
    if !(1..=MAX_PLANE_SUBDIVISIONS).contains(&subdivisions) {
        return Err(format!(
            "subdivisions must be 1..={MAX_PLANE_SUBDIVISIONS}, got {subdivisions}"
        ));
    }
    if mesh.primitive_type != "Plane" && subdivisions == 1 {
        // Clears a stray count; the shape ignores it anyway.
        mesh.subdivisions = 1;
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
    mesh.revision = primitive_revision(Primitive::Plane, subdivisions);
    mesh.own_geometry = None;
    Ok(())
}
