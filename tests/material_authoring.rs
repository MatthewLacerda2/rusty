//! `Material.DefineAsset` authors a reusable `MaterialAsset` by name into the per-World library: it round-trips through `SceneData`, an entity can use it by name, validation is single-sourced, map slots land correctly.

use std::cell::RefCell;
use std::rc::Rc;

use mlua::Lua;
use rusty::edit::{apply_scene_data, to_scene_data};
use rusty::scene::{MaterialComponent, RenderMode, Scene};
use rusty::script::api::bind;

fn run(scene: &Rc<RefCell<Scene>>, script: &str) {
    let lua = Lua::new();
    rusty::script::api::material::register(&lua, bind(&lua, scene)).unwrap();
    lua.load(script).exec().unwrap();
}

#[test]
fn defined_asset_round_trips_through_scene_data() {
    // Survives a save→load (to_scene_data → apply_scene_data): factors, map slots, render mode.
    let scene = Rc::new(RefCell::new(Scene::new()));
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
    assert_eq!(m.base_color_map, Some("brick_albedo.png".into()));
    assert_eq!(m.metallic_map, Some("brick_mr.png".into()));
    assert_eq!(m.roughness_map, Some("brick_mr.png".into()));
    assert_eq!(m.normal_map, Some("brick_n.png".into()));
    assert_eq!(m.emissive, [0.1, 0.0, 0.0]);
    assert_eq!(m.emissive_map, Some("brick_e.png".into()));
    assert_eq!(m.render_mode, RenderMode::Cutout);
    assert_eq!(m.alpha_cutoff, 0.4);
}

#[test]
fn validation_is_single_sourced_with_the_per_entity_path() {
    // Out-of-range alpha/cutoff clamp to [0,1], unknown render_mode degrades to the default, as the per-entity setters (shared `authoring::material::set_*`).
    let scene = Rc::new(RefCell::new(Scene::new()));
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
    let scene = Rc::new(RefCell::new(Scene::new()));
    run(
        &scene,
        r#"Material.DefineAsset("gold", { metallic = 1.0, roughness = 0.15,
            base_color = {1.0, 0.84, 0.0} })"#,
    );

    let id = {
        let mut sc = scene.borrow_mut();
        let id = sc.add_entity("Trophy".to_string());
        sc.world
            .set_material(id, Some(MaterialComponent::new("gold".to_string())));
        id
    };

    let sc = scene.borrow();
    let resolved = sc
        .material_asset_of(id)
        .expect("reference resolves to the asset");
    assert_eq!(resolved.metallic, 1.0);
    assert_eq!(resolved.roughness, 0.15);
    assert_eq!(resolved.base_color, [1.0, 0.84, 0.0]);
}

#[test]
fn packed_metallic_roughness_path_lands_in_both_map_slots() {
    // A packed metallic-roughness PNG (metallic→B, roughness→G) on both slots: each must carry the path.
    let scene = Rc::new(RefCell::new(Scene::new()));
    run(
        &scene,
        r#"Material.DefineAsset("packed", {
            metallic_map = "tile_mr.png", roughness_map = "tile_mr.png",
        })"#,
    );
    let scene = scene.borrow();
    let m = scene.materials.get("packed").unwrap();
    assert_eq!(m.metallic_map, Some("tile_mr.png".into()));
    assert_eq!(m.roughness_map, Some("tile_mr.png".into()));
}
