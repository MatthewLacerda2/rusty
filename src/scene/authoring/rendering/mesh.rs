//! src/scene/authoring/rendering/mesh.rs — the Mesh authoring op (#1524).
//!
//! A mesh's geometry comes from the model or primitive it was made from; the values
//! authored on it are a `Plane`'s subdivisions, the grid a vertex block
//! (`vertex_waves`) needs to move, how it casts shadows (#1571), its Scale In Lightmap
//! (#1704), and when a camera culls it as too far or too small (#1714). The Inspector's
//! Mesh card and `Component.Set` both write them here.
//!
//! Pure.

use crate::components::mesh::primitives::MAX_PLANE_SUBDIVISIONS;
use crate::components::{MeshComponent, ShadowCasting};
use crate::scene::authoring::sanitize::{self, clamped, non_negative};
use crate::scene::authoring::{primitive_revision, subdivided_geometry, Primitive};

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
    mesh.revision = primitive_revision(Primitive::Plane, subdivisions);
    Ok(())
}

/// Set how the mesh casts shadows (#1571): every shadow path reads it on its next
/// frame or bake, a static one's cached shadow included.
pub fn set_cast_shadows(mesh: &mut MeshComponent, mode: ShadowCasting) {
    mesh.cast_shadows = mode;
}

/// Set the distance in metres past which a camera stops drawing the mesh (#1714,
/// Unreal's *Max Draw Distance*); `0` is unlimited. Negative, NaN or infinite
/// is `0`.
pub fn set_max_draw_distance(mesh: &mut MeshComponent, metres: f32) {
    mesh.max_draw_distance = non_negative(metres);
}

/// Set the screen-relative height (`0..1`, Unity `LODGroup`'s measure) under which a
/// camera stops drawing the mesh (#1714); `0` is off. Held to `[0, 1]`; NaN or
/// infinite is `0`.
pub fn set_min_screen_size(mesh: &mut MeshComponent, height: f32) {
    mesh.min_screen_size = clamped(height, 0.0, 1.0);
}

/// What `Component.Get(id, "Mesh")` reports beyond the stored value: the
/// small-object cull's two settings, which the file leaves out while `0` (#1714),
/// so a script reads `0` there as `Mesh.Get*` does, never `nil`.
pub fn shown(m: &MeshComponent, value: &mut serde_json::Value) {
    value["max_draw_distance"] = serde_json::json!(m.max_draw_distance);
    value["min_screen_size"] = serde_json::json!(m.min_screen_size);
}

/// Set the mesh's Scale In Lightmap (#1704), held in
/// [`MeshComponent::SCALE_IN_LIGHTMAP`]; a non-finite value changes nothing. The
/// next bake reads it.
pub fn set_scale_in_lightmap(mesh: &mut MeshComponent, scale: f32) {
    let (lo, hi) = MeshComponent::SCALE_IN_LIGHTMAP.into_inner();
    mesh.scale_in_lightmap = sanitize::finite_or(scale, mesh.scale_in_lightmap).clamp(lo, hi);
}

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
            plane.revision != before,
            "a rebuilt grid takes its cut's own revision"
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
    fn the_small_object_cull_settings_are_held_in_range() {
        let mut m = primitive_mesh_component(Primitive::Box).unwrap();
        for (v, distance, size) in [(25.0, 25.0, 1.0), (0.05, 0.05, 0.05), (-3.0, 0.0, 0.0)] {
            set_max_draw_distance(&mut m, v);
            set_min_screen_size(&mut m, v);
            assert_eq!(
                (m.max_draw_distance, m.min_screen_size),
                (distance, size),
                "{v}"
            );
        }
        for bad in [f32::NAN, f32::INFINITY] {
            set_max_draw_distance(&mut m, bad);
            set_min_screen_size(&mut m, bad);
            assert_eq!(
                (m.max_draw_distance, m.min_screen_size),
                (0.0, 0.0),
                "{bad}"
            );
        }
        let mut saved = serde_json::to_value(&m).unwrap();
        assert!(saved.get("max_draw_distance").is_none(), "off is not saved");
        shown(&m, &mut saved);
        assert_eq!(saved["max_draw_distance"], 0.0, "but is read as 0");
        set_max_draw_distance(&mut m, 30.0);
        assert_eq!(serde_json::to_value(&m).unwrap()["max_draw_distance"], 30.0);
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

    #[test]
    fn scale_in_lightmap_is_held_in_its_range() {
        let mut cube = primitive_mesh_component(Primitive::Box).unwrap();
        assert_eq!(cube.scale_in_lightmap, 1.0);
        // A non-finite value changes nothing, so the 64 before it stands.
        let sets = [(2.5, 2.5), (-1.0, 0.0), (1e9, 64.0), (f32::NAN, 64.0)];
        for (set, held) in sets.into_iter().chain([(f32::INFINITY, 64.0)]) {
            set_scale_in_lightmap(&mut cube, set);
            assert_eq!(cube.scale_in_lightmap, held, "{set}");
        }
        set_scale_in_lightmap(&mut cube, 0.0);
        assert!(cube.reads_probes(), "0 reads light probes");
        assert_eq!(hold_scale_in_lightmap(&mut cube), None, "0 is in range");
        cube.scale_in_lightmap = 5000.0;
        assert!(hold_scale_in_lightmap(&mut cube)
            .unwrap()
            .contains("held at 64"));
        cube.scale_in_lightmap = f32::NAN;
        hold_scale_in_lightmap(&mut cube);
        assert_eq!(
            cube.scale_in_lightmap, 1.0,
            "a file's NaN reads the default"
        );
    }
}
