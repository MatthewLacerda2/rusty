use rusty_components::{MeshComponent, ShadowCasting};
pub use rusty_scene::rules::mesh::{hold_scale_in_lightmap, hold_subdivisions, set_subdivisions};
use rusty_scene::rules::sanitize::{self, clamped, non_negative};

pub fn set_cast_shadows(mesh: &mut MeshComponent, mode: ShadowCasting) {
    mesh.cast_shadows = mode;
}

/// Unreal's *Max Draw Distance*; `0` is unlimited.
pub fn set_max_draw_distance(mesh: &mut MeshComponent, metres: f32) {
    mesh.max_draw_distance = non_negative(metres);
}

/// Unity `LODGroup`'s measure; `0` is off.
pub fn set_min_screen_size(mesh: &mut MeshComponent, height: f32) {
    mesh.min_screen_size = clamped(height, 0.0, 1.0);
}

/// The small-object cull's two settings, which the file leaves out while `0`, so a script reads `0`
/// there, never `nil`.
pub fn shown(m: &MeshComponent, value: &mut serde_json::Value) {
    value["max_draw_distance"] = serde_json::json!(m.max_draw_distance);
    value["min_screen_size"] = serde_json::json!(m.min_screen_size);
}

pub fn set_scale_in_lightmap(mesh: &mut MeshComponent, scale: f32) {
    let (lo, hi) = MeshComponent::SCALE_IN_LIGHTMAP.into_inner();
    mesh.scale_in_lightmap = sanitize::finite_or(scale, mesh.scale_in_lightmap).clamp(lo, hi);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::authoring::{primitive_mesh_component, Primitive};
    use rusty_components::mesh::primitives::MAX_PLANE_SUBDIVISIONS;

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
        cube.subdivisions = 4;
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
