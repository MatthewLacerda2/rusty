//! Material library (#201): materials are reusable assets in the scene's library,
//! entities reference one by name. Covers sharing, round-trip, and back-compat with
//! pre-#201 scenes that stored the material inline as `texture`.

use std::cell::RefCell;

use mlua::Lua;
use rusty::scene::authoring::material as mat_ops;
use rusty::scene::authoring::{add_component, create_entity, ComponentKind};
use rusty::scene::{
    apply_scene_data, to_scene_data, MaterialAsset, MaterialComponent, RenderMode, Scene,
};

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
    scene.get_entity_mut(id).unwrap().material = Some(MaterialComponent {
        material: key.to_string(),
    });
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
    let ea = scene.get_entity(a).unwrap();
    let eb = scene.get_entity(b).unwrap();
    assert_eq!(scene.material_of(&ea).unwrap().base_color, [0.1, 0.2, 0.3]);
    assert_eq!(scene.material_of(&eb).unwrap().base_color, [0.1, 0.2, 0.3]);
    assert_eq!(scene.materials.len(), 1, "one shared asset, not per-entity");
}

#[test]
fn authoring_add_creates_library_asset_remove_keeps_it() {
    let mut scene = Scene::new();
    let id = create_entity(&mut scene, "Box", None);
    // Add creates the shared library asset AND attaches the reference.
    add_component(&mut scene, id, ComponentKind::Texture);
    assert_eq!(scene.materials.len(), 1);
    assert!(scene.get_entity(id).unwrap().material.is_some());
    // Remove drops only the reference; the library asset is left for other users.
    rusty::scene::authoring::remove_component(&mut scene, id, ComponentKind::Texture);
    assert!(scene.get_entity(id).unwrap().material.is_none());
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

    let e = loaded.get_entity(id).unwrap();
    let resolved = loaded
        .material_of(&e)
        .expect("reference resolves post-load");
    assert_eq!(resolved.base_color, [0.5, 0.25, 0.125]);
    assert_eq!(
        resolved.base_color_map.as_deref(),
        Some("textures/wood.png")
    );
    assert_eq!(resolved.metallic, 0.8);
    assert_eq!(resolved.roughness, 0.2);
    assert_eq!(e.material.as_ref().unwrap().material, "wood");
}

#[test]
fn lua_material_api_and_shared_op_converge() -> Result<(), Box<dyn std::error::Error>> {
    // #287: the Lua `Material.*` bindings and the editor's Material card both route
    // through the SAME `authoring::material::*` ops, so applying the same change either
    // way must yield byte-identical `MaterialAsset` state. We pin convergence by
    // driving the Lua surface against one entity and the shared op directly against a
    // second, then asserting the two resolved materials are equal. The clamp on
    // `SetAlpha`/`SetAlphaCutoff` is exercised (2.0 / -1.0 -> 1.0 / 0.0), proving the
    // validation is single-sourced in the shared op the binding calls.
    let scene = RefCell::new(Scene::new());
    let via_lua = scene.borrow_mut().add_entity("ViaLua".to_string());
    let via_op = scene.borrow_mut().add_entity("ViaOp".to_string());

    let lua = Lua::new();
    lua.scope(|s| {
        rusty::api::material::register(&lua, s, &scene).unwrap();
        lua.load(format!(
            r#"
            Material.SetMetallic({via_lua}, 0.7)
            Material.SetRoughness({via_lua}, 0.2)
            Material.SetEmissive({via_lua}, {{1.0, 0.5, 0.0}})
            Material.SetTexture({via_lua}, "albedo.png")
            Material.SetRenderMode({via_lua}, "transparent")
            Material.SetAlpha({via_lua}, 2.0)
            Material.SetAlphaCutoff({via_lua}, -1.0)
        "#
        ))
        .exec()
        .unwrap();
        Ok(())
    })?;

    // The shared op path: resolve-or-create the key (same verb the API uses), then call
    // the same ops the editor card calls, with the same inputs.
    {
        let mut sc = scene.borrow_mut();
        let key = mat_ops::ensure_material_key(&mut sc, via_op).unwrap();
        mat_ops::set_metallic(&mut sc.materials, &key, 0.7);
        mat_ops::set_roughness(&mut sc.materials, &key, 0.2);
        mat_ops::set_emissive(&mut sc.materials, &key, [1.0, 0.5, 0.0]);
        mat_ops::set_base_color_map(&mut sc.materials, &key, "albedo.png".to_string());
        mat_ops::set_render_mode(&mut sc.materials, &key, RenderMode::Transparent);
        mat_ops::set_alpha(&mut sc.materials, &key, 2.0);
        mat_ops::set_alpha_cutoff(&mut sc.materials, &key, -1.0);
    }

    let sc = scene.borrow();
    let lua_mat = sc.material_of(&sc.get_entity(via_lua).unwrap()).unwrap();
    let op_mat = sc.material_of(&sc.get_entity(via_op).unwrap()).unwrap();

    assert_eq!(lua_mat.metallic, op_mat.metallic);
    assert_eq!(lua_mat.roughness, op_mat.roughness);
    assert_eq!(lua_mat.emissive, op_mat.emissive);
    assert_eq!(lua_mat.base_color_map, op_mat.base_color_map);
    assert_eq!(lua_mat.render_mode, op_mat.render_mode);
    // Both clamped identically by the single shared op.
    assert_eq!(lua_mat.alpha, 1.0);
    assert_eq!(op_mat.alpha, 1.0);
    assert_eq!(lua_mat.alpha_cutoff, 0.0);
    assert_eq!(op_mat.alpha_cutoff, 0.0);
    Ok(())
}

/// An OLD-format scene (entity with inline `"texture"`, no `materials`) as JSON.
/// `color`/`path` set the migrated values; the rest are minimal valid fields.
fn legacy_scene_json() -> String {
    r#"{
        "entities": [{
            "id": 7, "name": "OldBox", "active": true, "is_static": false,
            "transform": { "position": [0,0,0], "rotation": [0,0,0,1], "scale": [1,1,1] },
            "mesh": null,
            "texture": { "path": "tex/brick.png", "is_dirty": false, "metallic": 0.3,
                "roughness": 0.7, "metallic_map": null, "roughness_map": null,
                "color": [0.2, 0.4, 0.6] },
            "scripts": [], "parent_id": null, "children": []
        }],
        "next_entity_id": 8, "selected_entity_id": null
    }"#
    .to_string()
}

#[test]
fn legacy_inline_texture_migrates_into_library_material() {
    // A pre-#201 inline `texture` must load with no data loss into a library
    // material + a synthesized reference.
    let data: rusty::scene::SceneData =
        serde_json::from_str(&legacy_scene_json()).expect("legacy scene parses");
    let mut scene = Scene::new();
    apply_scene_data(&mut scene, data);

    let e = scene.get_entity(7).unwrap();
    let key = &e.material.as_ref().expect("reference attached").material;
    assert_eq!(key, "entity_7_material");
    let mat = scene.material_of(&e).expect("legacy material migrated");
    assert_eq!(mat.base_color, [0.2, 0.4, 0.6]);
    assert_eq!(mat.base_color_map.as_deref(), Some("tex/brick.png"));
    assert_eq!(mat.metallic, 0.3);
    assert_eq!(mat.roughness, 0.7);
    assert!(
        e.pending_material.is_none(),
        "carrier cleared after migration"
    );
}

#[test]
fn tracked_default_scene_and_corpus_seeds_still_load() {
    // The tracked default.scene + full-document corpus seed are OLD-format (inline
    // `texture`, no `materials`); both MUST load via the legacy migration without
    // panicking, proving the back-compat path against the real checked-in files.
    for path in [
        "assets/scenes/default.scene",
        "fuzz/corpus/scene_deserialize/seed_default.scene",
    ] {
        let text = std::fs::read_to_string(path).unwrap();
        let data: rusty::scene::SceneData =
            serde_json::from_str(&text).unwrap_or_else(|e| panic!("{path}: {e}"));
        apply_scene_data(&mut Scene::new(), data);
    }
}
