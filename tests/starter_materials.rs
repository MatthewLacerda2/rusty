//! Starter material library (#173): the engine ships a small set of ready-to-go
//! glTF-PBR materials under `project/materials/starter.gltf`. They must be
//! discoverable and referable from scenes the SAME way user/imported materials are
//! — no engine special-casing. This test drives the ordinary asset path: the
//! manifest discovers them, the importer reads their PBR factors, and they round-trip
//! through `SceneData` as named library entries an entity references by key.

use rusty::asset::{build_manifest, import_file};
use rusty::scene::authoring::material_asset_from_import;
use rusty::scene::{apply_scene_data, to_scene_data, MaterialComponent, Scene};
use std::path::Path;

const STARTER: &str = "project/materials/starter.gltf";

/// The four starter materials and the physically-plausible factors they ship with.
/// (name, metallic, roughness, emits-light)
const EXPECTED: &[(&str, f32, f32, bool)] = &[
    ("Matte", 0.0, 0.9, false),
    ("Metal", 1.0, 0.25, false),
    ("Plastic", 0.0, 0.4, false),
    ("Emissive", 0.0, 0.6, true),
];

#[test]
fn starter_materials_are_discoverable_via_manifest() {
    // `Assets.Manifest()` walks `project/`; the starter file must show up there,
    // exposing one material per addressable sub-object — exactly like any user model.
    let manifest = build_manifest(Path::new("project"));
    let entry = manifest
        .assets
        .iter()
        .find(|a| a.path.replace('\\', "/") == STARTER)
        .expect("starter.gltf is discovered under the project asset root");
    assert_eq!(
        entry.material_count,
        EXPECTED.len(),
        "every starter material is surfaced in the file's material table"
    );
    for (name, ..) in EXPECTED {
        assert!(
            entry.sub_objects.iter().any(|s| &s.id == name),
            "{name} is an addressable sub-object the agent can place"
        );
    }
}

#[test]
fn starter_materials_resolve_as_library_references_round_trip() {
    // Build library entries the SAME way the editor's glTF import does: key each by
    // the deterministic `<path>::<name>`, convert via `material_asset_from_import`,
    // and attach a thin `MaterialComponent` reference — the user-material path.
    let asset = import_file(Path::new(STARTER)).expect("starter.gltf imports");
    let mut scene = Scene::new();
    let mut ids = Vec::new();
    for data in &asset.materials {
        let key = format!("{STARTER}::{}", data.name);
        scene
            .materials
            .insert(key.clone(), material_asset_from_import(data));
        let id = scene.add_entity(data.name.clone());
        scene.get_entity_mut(id).unwrap().material = Some(MaterialComponent { material: key });
        ids.push((id, data.name.clone()));
    }

    // Round-trip through the scene document (no GPU buffers), then re-resolve.
    let document = to_scene_data(&scene);
    let mut loaded = Scene::new();
    apply_scene_data(&mut loaded, document);

    for (id, name) in &ids {
        let (_, metallic, roughness, emits) = EXPECTED
            .iter()
            .find(|(n, ..)| n == name)
            .unwrap_or_else(|| panic!("unexpected starter material {name}"));
        let entity = loaded.get_entity(*id).unwrap();
        assert_eq!(
            entity.material.as_ref().unwrap().material,
            format!("{STARTER}::{name}"),
            "{name} keeps its deterministic library key across save/load"
        );
        let resolved = loaded
            .material_of(&entity)
            .unwrap_or_else(|| panic!("{name} reference resolves post-load"));
        assert_eq!(resolved.metallic, *metallic, "{name} metallic");
        assert_eq!(resolved.roughness, *roughness, "{name} roughness");
        assert_eq!(
            resolved.emissive != [0.0, 0.0, 0.0],
            *emits,
            "{name} emission"
        );
    }
}
