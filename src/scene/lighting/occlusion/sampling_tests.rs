//! What the bake samples (#1714): a wall of any length seen through a small gap
//! is kept, an object too large to sample is never culled, a re-exported mesh
//! with the same counts is a new fingerprint, and a mesh drawing no triangle takes
//! no part.

use glam::Vec3;

use super::test_gap::gap;
use super::test_level::{block, corridors};
use super::{bake_occlusion, culls_from, fingerprint, MAX_CULLING_NEAR};
use crate::components::mesh::MeshRevision;
use crate::components::{MaterialComponent, Projection};

/// Where [`gap`]'s doors stand along the wall, off any regular spacing.
const DOORS: [f32; 4] = [0.0, 3.37, -6.91, 8.53];

#[test]
fn a_long_wall_seen_through_a_door_is_kept_at_any_length() {
    let cases = [20.0, 40.0, 200.0]
        .into_iter()
        .flat_map(|l| DOORS.map(|x| (l, x)));
    for (length, x) in cases {
        let (scene, door, wall) = gap(length, x);
        let data = bake_occlusion(&scene, 4)
            .0
            .expect("the front wall occludes");
        let eye = Vec3::new(x, 1.5, 20.0);
        let row = data.row_at(eye).expect("the eye is in the grid");
        let seen = |id| data.sees(row, data.object_of(id).expect("baked"));
        assert!(
            seen(door),
            "{length} m, door at {x}: the door's sill is in sight"
        );
        assert!(
            seen(wall),
            "{length} m, door at {x}: the wall shows through the door"
        );
        // Sampled, not kept by size: away from the door some cell hides it.
        let object = data.object_of(wall).expect("baked");
        let hidden = (0..data.row_count() as u32).any(|r| !data.sees(r, object));
        assert!(
            hidden,
            "{length} m: the wall is culled where no door shows it"
        );
    }
}

#[test]
fn an_object_too_large_to_sample_is_never_culled() {
    // 5 km of wall behind the door: past the target cap, so it is kept everywhere.
    let (scene, _, wall) = gap(5000.0, 0.0);
    let (data, report) = bake_occlusion(&scene, 4);
    let data = data.expect("occluders");
    assert!(report.never_culled >= 1, "{report:?}");
    let object = data.object_of(wall).expect("baked");
    assert!((0..data.row_count() as u32).all(|row| data.sees(row, object)));
}

#[test]
fn a_reexport_that_moves_a_vertex_is_a_new_fingerprint() {
    let mut scene = corridors();
    let id = scene.world.ids_with_mesh()[3];
    let before = fingerprint(&scene, id).expect("a static mesh");
    // The same vertices under a new revision (a re-import of an unchanged file).
    scene.world.mesh_mut(id).unwrap().revision = MeshRevision::next();
    assert_eq!(
        fingerprint(&scene, id),
        Some(before),
        "the revision is only a key"
    );
    // A doorway moved in Blender: the same counts, one vertex elsewhere.
    let mut mesh = scene.world.mesh_mut(id).unwrap();
    mesh.vertices[0].position[0] += 0.5;
    mesh.revision = MeshRevision::next();
    drop(mesh);
    assert_ne!(fingerprint(&scene, id), Some(before));
}

#[test]
fn a_static_mesh_drawing_no_triangle_takes_no_part() {
    let mut scene = corridors();
    let empty = block(&mut scene, "Empty", Vec3::splat(2.0), Vec3::splat(3.0));
    let none = MaterialComponent { materials: vec![] };
    scene.world.set_material(empty, Some(none));
    assert_eq!(fingerprint(&scene, empty), None);
    let data = bake_occlusion(&scene, 2)
        .0
        .expect("no panic, the rest bakes");
    assert_eq!(data.object_of(empty), None);
}

#[test]
fn a_level_of_only_transparent_statics_reports_nothing() {
    let mut scene = corridors();
    let glass = crate::components::MaterialAsset {
        render_mode: crate::components::RenderMode::Transparent,
        ..Default::default()
    };
    scene.materials.insert("glass".into(), glass);
    for id in scene.world.ids_with_mesh() {
        scene
            .world
            .set_material(id, Some(MaterialComponent::new("glass")));
    }
    let (data, report) = bake_occlusion(&scene, 2);
    assert_eq!((data, report), (None, Default::default()));
}

#[test]
fn only_a_perspective_camera_with_a_near_plane_close_in_culls() {
    assert!(culls_from(Projection::Perspective, 0.1));
    assert!(culls_from(Projection::Perspective, MAX_CULLING_NEAR));
    assert!(
        !culls_from(Projection::Perspective, 5.0),
        "a far near plane"
    );
    let ortho = Projection::Orthographic { size: 20.0 };
    assert!(!culls_from(ortho, 0.1), "an orthographic minimap");
}
