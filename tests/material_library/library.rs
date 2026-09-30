//! Sharing one library material, authoring add/remove, and the scene-data round trip.

use rusty::scene::authoring::{add_component, create_entity, ComponentKind};
use rusty::scene::{apply_scene_data, to_scene_data, MaterialAsset, MaterialComponent, Scene};

/// Build an entity referencing `key`, inserting `key`'s material if `material` is
/// `Some`. Returns the new entity id.
fn entity_referencing(
    scene: &mut Scene,
    name: &str,
    key: &str,
    material: Option<MaterialAsset>,
) -> u32 {
    if let Some(mat) = material {
        scene.materials.insert(key.to_string(), mat);
    }
    let id = scene.add_entity(name.to_string());
    scene.world.set_material(
        id,
        Some(MaterialComponent {
            material: key.to_string(),
        }),
    );
    id
}

#[test]
fn two_entities_share_one_library_material() {
    let mut scene = Scene::new();
    let a = entity_referencing(&mut scene, "A", "shared", Some(MaterialAsset::default()));
    let b = entity_referencing(&mut scene, "B", "shared", None);

    // Edit the shared material once.
    scene.materials.get_mut("shared").unwrap().base_color = [0.1, 0.2, 0.3];

    // Both entities resolve to the same edited asset (sharing works).
    assert_eq!(
        scene.material_asset_of(a).unwrap().base_color,
        [0.1, 0.2, 0.3]
    );
    assert_eq!(
        scene.material_asset_of(b).unwrap().base_color,
        [0.1, 0.2, 0.3]
    );
    assert_eq!(scene.materials.len(), 1, "one shared asset, not per-entity");
}

#[test]
fn authoring_add_creates_library_asset_remove_keeps_it() {
    let mut scene = Scene::new();
    let id = create_entity(&mut scene, "Box", None);
    // Add creates the shared library asset AND attaches the reference.
    add_component(&mut scene, id, ComponentKind::Texture);
    assert_eq!(scene.materials.len(), 1);
    assert!(scene.world.has_material(id));
    // Remove drops only the reference; the library asset is left for other users.
    rusty::scene::authoring::remove_component(&mut scene, id, ComponentKind::Texture);
    assert!(!scene.world.has_material(id));
    assert_eq!(
        scene.materials.len(),
        1,
        "shared asset stays in the library"
    );
}

#[test]
fn material_and_reference_survive_scene_data_round_trip() {
    let mut scene = Scene::new();
    let mat = MaterialAsset {
        base_color: [0.5, 0.25, 0.125],
        base_color_map: Some("textures/wood.png".to_string()),
        metallic: 0.8,
        roughness: 0.2,
        ..MaterialAsset::default()
    };
    let id = entity_referencing(&mut scene, "Crate", "wood", Some(mat));

    // to_scene_data -> apply_scene_data carries the library + the reference, and
    // never any GPU buffers (it is the same document a save writes).
    let data = to_scene_data(&scene);
    let mut loaded = Scene::new();
    apply_scene_data(&mut loaded, data);

    let resolved = loaded
        .material_asset_of(id)
        .expect("reference resolves post-load");
    assert_eq!(resolved.base_color, [0.5, 0.25, 0.125]);
    assert_eq!(
        resolved.base_color_map.as_deref(),
        Some("textures/wood.png")
    );
    assert_eq!(resolved.metallic, 0.8);
    assert_eq!(resolved.roughness, 0.2);
    assert_eq!(loaded.world.material(id).unwrap().material, "wood");
}
