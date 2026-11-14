//! A fingerprint is what the bake reads and nothing else (#1958): a scene loaded
//! straight after its own bake matches it, however the bake's process differed
//! from the one loading it, while a real change still breaks it.

use std::sync::Arc;

use glam::Vec3;

use super::test_level::corridors;
use super::{bake_occlusion, fingerprint, save_sidecar};
use rusty_scene::authoring::material::define_asset;
use rusty_scene::{MaterialAsset, MaterialComponent, MeshRevision, Scene};
use rusty_components::{RenderFace, RenderMode};

/// The case from the game: every static mesh draws with `"wood"`, which the
/// scene file's library lacks (the game's script defines it). A headless run
/// defines it in memory, bakes and writes the PVS; a fresh load, the editor's,
/// never ran that script. Before #1958 every baked mesh read as changed.
#[test]
fn a_fresh_load_matches_a_bake_that_defined_a_material_in_memory() {
    let dir = rusty_test_support::temp::dir().join("occlusion_fresh_load");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("level.scene").to_string_lossy().into_owned();
    let mut level = corridors();
    for id in level.world.ids_with_mesh() {
        let wood = Some(MaterialComponent::new("wood"));
        level.world.set_material(id, wood);
    }
    level.materials.remove("wood");
    level.save_to_file(&path).unwrap();

    let mut baking = Scene::new();
    baking.load_from_file(&path).unwrap();
    define_asset(&mut baking.materials, "wood", MaterialAsset::default());
    let data = bake_occlusion(&baking, 2).0.expect("occluders");
    *baking.occlusion = Some(Arc::new(data));
    save_sidecar(&baking, &path).unwrap();

    let mut editor = Scene::new();
    editor.load_from_file(&path).unwrap();
    let pvs = editor.occlusion.as_deref().expect("the bake loaded");
    assert!(pvs.objects.iter().any(|o| o.occluder));
    for baked in &pvs.objects {
        let now = fingerprint(&editor, baked.entity);
        assert_eq!(now, Some(baked.fingerprint), "entity {}", baked.entity);
    }
}

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
    mesh.vertices = corners;
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
    scene.world.mesh_mut(id).unwrap().vertices[0].position[1] += 0.01;
    scene.world.mesh_mut(id).unwrap().revision = MeshRevision::next();
    assert_ne!(fingerprint(&scene, id), Some(before), "a corner moved");
}

#[test]
fn a_material_that_occludes_differently_breaks_the_print_and_a_renamed_one_does_not() {
    let mut scene = corridors();
    let id = scene.world.ids_with_mesh()[0];
    let before = fingerprint(&scene, id).expect("a static mesh");
    define_asset(&mut scene.materials, "stone", MaterialAsset::default());
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
        define_asset(&mut scene.materials, "stone", asset);
        assert_ne!(fingerprint(&scene, id), Some(before));
    }
    scene.world.transform_mut(id).unwrap().position += Vec3::X * 0.01;
    define_asset(&mut scene.materials, "stone", MaterialAsset::default());
    assert_ne!(fingerprint(&scene, id), Some(before), "moved");
}
