//! Material authoring (#271): author a reusable `MaterialAsset` into the per-World
//! library by NAME (decoupled from any entity) via the `Material.DefineAsset` surface,
//! and prove the issue's contract — the asset round-trips through `SceneData`, an entity
//! can use it by name, validation is single-sourced, and map slots land correctly.

use std::cell::RefCell;

use mlua::Lua;
use rusty::scene::{apply_scene_data, to_scene_data, MaterialComponent, RenderMode, Scene};

/// Register the `Material` namespace against `scene` and run `script`.
fn run(scene: &RefCell<Scene>, script: &str) {
    let lua = Lua::new();
    lua.scope(|s| {
        rusty::api::material::register(&lua, s, scene).unwrap();
        lua.load(script).exec().unwrap();
        Ok(())
    })
    .unwrap();
}

#[test]
fn defined_asset_round_trips_through_scene_data() {
    // DefineAsset writes the asset into scene.materials by name; a save→load (the same
    // to_scene_data → apply_scene_data document a `Scene.Save` writes) must preserve its
    // factors, map slots, and render mode — the round-trip the issue requires.
    let scene = RefCell::new(Scene::new());
    run(
        &scene,
        r#"Material.DefineAsset("brick", {
            base_color = {0.5, 0.25, 0.125},
            metallic = 0.0, roughness = 0.8,
            base_color_map = "brick_albedo.png",
            metallic_map = "brick_mr.png", roughness_map = "brick_mr.png",
            normal_map = "brick_n.png", emissive = {0.1, 0.0, 0.0},
            emissive_map = "brick_e.png",
            render_mode = "Cutout", alpha = 1.0, alpha_cutoff = 0.4,
        })"#,
    );

    let data = to_scene_data(&scene.borrow());
    let mut loaded = Scene::new();
    apply_scene_data(&mut loaded, data);

    let m = loaded.materials.get("brick").expect("asset survives load");
    assert_eq!(m.base_color, [0.5, 0.25, 0.125]);
    assert_eq!(m.roughness, 0.8);
    assert_eq!(m.base_color_map.as_deref(), Some("brick_albedo.png"));
    assert_eq!(m.metallic_map.as_deref(), Some("brick_mr.png"));
    assert_eq!(m.roughness_map.as_deref(), Some("brick_mr.png"));
    assert_eq!(m.normal_map.as_deref(), Some("brick_n.png"));
    assert_eq!(m.emissive, [0.1, 0.0, 0.0]);
    assert_eq!(m.emissive_map.as_deref(), Some("brick_e.png"));
    assert_eq!(m.render_mode, RenderMode::Cutout);
    assert_eq!(m.alpha_cutoff, 0.4);
}

#[test]
fn validation_is_single_sourced_with_the_per_entity_path() {
    // Out-of-range alpha/cutoff clamp to [0,1] and an unknown render_mode degrades to the
    // safe default — identical to the per-entity setters, because DefineAsset routes the
    // validated fields through the same shared `authoring::material::set_*` ops.
    let scene = RefCell::new(Scene::new());
    run(
        &scene,
        r#"Material.DefineAsset("bad", {
            render_mode = "hologram", alpha = 5.0, alpha_cutoff = -3.0,
        })"#,
    );
    let scene = scene.borrow();
    let m = scene.materials.get("bad").unwrap();
    assert_eq!(m.render_mode, RenderMode::Opaque, "unknown mode → default");
    assert_eq!(m.alpha, 1.0, "alpha clamps to upper bound");
    assert_eq!(m.alpha_cutoff, 0.0, "cutoff clamps to lower bound");
}

#[test]
fn entity_uses_the_named_asset_by_reference() {
    // The day-one consumer: define an asset standalone, then point an entity's
    // MaterialComponent at the name. `material_of` must resolve to the authored asset.
    let scene = RefCell::new(Scene::new());
    run(
        &scene,
        r#"Material.DefineAsset("gold", { metallic = 1.0, roughness = 0.15,
            base_color = {1.0, 0.84, 0.0} })"#,
    );

    let id = {
        let mut sc = scene.borrow_mut();
        let id = sc.add_entity("Trophy".to_string());
        sc.get_entity_mut(id).unwrap().material = Some(MaterialComponent {
            material: "gold".to_string(),
        });
        id
    };

    let sc = scene.borrow();
    let e = sc.get_entity(id).unwrap();
    let resolved = sc.material_of(&e).expect("reference resolves to the asset");
    assert_eq!(resolved.metallic, 1.0);
    assert_eq!(resolved.roughness, 0.15);
    assert_eq!(resolved.base_color, [1.0, 0.84, 0.0]);
}

#[test]
fn packed_metallic_roughness_path_lands_in_both_map_slots() {
    // A packed glTF metallic-roughness PNG (metallic→B, roughness→G) is pointed at both
    // the metallic_map and roughness_map slots; each must carry the path so the renderer
    // samples the right channel from each.
    let scene = RefCell::new(Scene::new());
    run(
        &scene,
        r#"Material.DefineAsset("packed", {
            metallic_map = "tile_mr.png", roughness_map = "tile_mr.png",
        })"#,
    );
    let scene = scene.borrow();
    let m = scene.materials.get("packed").unwrap();
    assert_eq!(m.metallic_map.as_deref(), Some("tile_mr.png"));
    assert_eq!(m.roughness_map.as_deref(), Some("tile_mr.png"));
}
