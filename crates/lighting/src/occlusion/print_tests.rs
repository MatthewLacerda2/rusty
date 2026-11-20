//! A fingerprint is what the bake reads and nothing else (#1958): the same
//! triangles stored another way and a material renamed keep it, while a real change
//! still breaks it. A scene loaded straight after its own bake matching it is the
//! integration suite's (`tests/occlusion/fresh_load.rs`): it saves and loads.

use glam::Vec3;

use crate::occlusion::fingerprint;
use crate::occlusion::test_level::corridors;
use rusty_components::{RenderFace, RenderMode};
use rusty_scene::{MaterialAsset, MaterialComponent, MeshRevision, Scene};

/// `id`'s mesh with every corner its own vertex: the triangles of a re-export
/// that welds nothing, under a new revision.
fn unwelded(scene: &mut Scene, id: u32) {
    let mut mesh = scene.world.mesh_mut(id).unwrap();
    let corners: Vec<_> = mesh
        .indices
        .iter()
        .map(|&i| mesh.vertices[i as usize])
        .collect();
    mesh.indices = (0..corners.len() as u32).collect();
    mesh.vertices = corners.into();
    mesh.revision = MeshRevision::next();
}

#[test]
fn the_same_triangles_stored_another_way_keep_the_print_and_a_moved_corner_does_not() {
    let mut scene = corridors();
    let id = scene.world.ids_with_mesh()[0];
    let before = fingerprint(&scene, id).expect("a static mesh");
    unwelded(&mut scene, id);
    assert_eq!(
        fingerprint(&scene, id),
        Some(before),
        "only the buffer moved"
    );
    scene.world.mesh_mut(id).unwrap().vertices_mut()[0].position[1] += 0.01;
    assert_ne!(fingerprint(&scene, id), Some(before), "a corner moved");
}

#[test]
fn a_material_that_occludes_differently_breaks_the_print_and_a_renamed_one_does_not() {
    let mut scene = corridors();
    let id = scene.world.ids_with_mesh()[0];
    let before = fingerprint(&scene, id).expect("a static mesh");
    scene
        .materials
        .insert("stone".into(), MaterialAsset::default());
    scene
        .world
        .set_material(id, Some(MaterialComponent::new("stone")));
    assert_eq!(
        fingerprint(&scene, id),
        Some(before),
        "an opaque front face still"
    );
    let cases = [
        MaterialAsset {
            render_mode: RenderMode::Cutout,
            ..MaterialAsset::default()
        },
        MaterialAsset {
            render_face: RenderFace::Both,
            ..MaterialAsset::default()
        },
    ];
    for asset in cases {
        scene.materials.insert("stone".into(), asset);
        assert_ne!(fingerprint(&scene, id), Some(before));
    }
    scene.world.transform_mut(id).unwrap().position += Vec3::X * 0.01;
    scene
        .materials
        .insert("stone".into(), MaterialAsset::default());
    assert_ne!(fingerprint(&scene, id), Some(before), "moved");
}
