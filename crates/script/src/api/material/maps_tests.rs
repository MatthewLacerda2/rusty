use std::cell::RefCell;
use std::rc::Rc;

use mlua::Lua;

use crate::api::bind;
use rusty_asset::MapRef;
use rusty_edit::authoring::material::MAPS_DIR;
use rusty_scene::Scene;

const MAPS: &str = r#"{ resolution = 8, seed = 3,
    nodes = { { id = "c", op = "checker", tiles = 2,
                color_a = {0,0,0,1}, color_b = {1,1,1,1} } },
    outputs = { base_color = "c", metallic_roughness = "c" } }"#;

fn run(scene: &Rc<RefCell<Scene>>, script: &str) -> Result<(), String> {
    let lua = Lua::new();
    crate::api::attempt(|| {
        super::super::register(&lua, bind(&lua, scene)).unwrap();
        Ok(lua.load(script).exec())
    })
    .unwrap()
    .map_err(|e| e.to_string())
}

fn names(test: &str) -> (String, impl Fn(&str) -> String) {
    let name = format!("test409_{test}_{}", std::process::id());
    let prefix = format!("{MAPS_DIR}/{name}");
    (name, move |slot: &str| format!("{prefix}_{slot}.png"))
}

fn cleanup(name: &str) {
    for slot in ["base_color", "metallic_roughness"] {
        std::fs::remove_file(format!("{MAPS_DIR}/{name}_{slot}.png")).ok();
    }
}

#[test]
fn define_asset_bakes_maps_and_fills_the_map_paths() {
    let scene = Rc::new(RefCell::new(Scene::new()));
    let (name, path) = names("define");
    let result = run(
        &scene,
        &format!(r#"Material.DefineAsset("{name}", {{ metallic = 1.0, maps = {MAPS} }})"#),
    );
    let m = scene.borrow().materials.get(&name).cloned();
    cleanup(&name);
    result.unwrap();
    let m = m.expect("asset defined");
    assert_eq!(m.base_color_map, Some(path("base_color").into()));
    assert_eq!(m.metallic_map, Some(path("metallic_roughness").into()));
    assert_eq!(m.roughness_map, Some(path("metallic_roughness").into()));
    assert_eq!(m.metallic, 1.0, "factors still scale the maps");
    assert_eq!(m.maps_recipe.unwrap().seed, 3, "the recipe rides the asset");
}

#[test]
fn rebake_takes_a_new_resolution_and_get_asset_returns_the_recipe() {
    let scene = Rc::new(RefCell::new(Scene::new()));
    let (name, path) = names("rebake");
    let result = run(
        &scene,
        &format!(
            r#"Material.DefineAsset("{name}", {{ maps = {MAPS} }})
               Material.Rebake("{name}", 16)
               local doc = Material.GetAsset("{name}")
               assert(doc:find('"maps"'), doc)
               Material.DefineAsset("{name}_copy", doc)"#
        ),
    );
    let width = image::image_dimensions(path("base_color")).map(|d| d.0);
    cleanup(&name);
    cleanup(&format!("{name}_copy"));
    result.unwrap();
    assert_eq!(width.unwrap(), 16);
    let scene = scene.borrow();
    let copy = &scene.materials[&format!("{name}_copy")];
    assert_eq!(copy.maps_recipe.as_ref().unwrap().resolution, 16);
}

#[test]
fn a_bad_maps_recipe_defines_nothing() {
    let scene = Rc::new(RefCell::new(Scene::new()));
    let err = run(
        &scene,
        r#"Material.DefineAsset("bad", { maps = { resolution = 8,
              nodes = { { id = "w", op = "white_noise" } },
              outputs = { basecolour = "w" } } })"#,
    )
    .unwrap_err();
    assert!(err.contains("basecolour"), "{err}");
    assert!(!scene.borrow().materials.contains_key("bad"));

    let err = run(
        &scene,
        r#"Material.DefineAsset("bad", { maps = { nodez = {} } })"#,
    )
    .unwrap_err();
    assert!(err.contains("maps is a texture recipe"), "{err}");
    let err = run(&scene, r#"Material.Rebake("absent")"#).unwrap_err();
    assert!(err.contains("no material named"), "{err}");
}

#[test]
fn the_occlusion_and_specular_maps_set_through_lua_and_a_recipe() {
    let scene = Rc::new(RefCell::new(Scene::new()));
    let id = scene.borrow_mut().add_entity("E".into());
    let lua = format!(
        r#"Material.SetOcclusionMap({id}, "ao.png") Material.SetSpecularMap({id}, "s.png")
           Material.SetOcclusionStrength({id}, 4)
           Material.DefineAsset("felt", {{ occlusion_map = "orm.png", occlusion_strength = 0.5 }})"#
    );
    run(&scene, &lua).unwrap();
    let sc = scene.borrow();
    let path = |m: &Option<MapRef>| m.as_ref().map(|p| p.to_string());
    let felt = &sc.materials["felt"];
    assert_eq!(
        (path(&felt.occlusion_map), felt.occlusion_strength),
        (Some("orm.png".into()), 0.5)
    );
    let own = sc
        .materials
        .values()
        .find(|m| path(&m.specular_map) == Some("s.png".into()));
    let own = own.expect("the entity's material took the specular map");
    assert_eq!(
        (path(&own.occlusion_map), own.occlusion_strength),
        (Some("ao.png".into()), 1.0)
    );
}
