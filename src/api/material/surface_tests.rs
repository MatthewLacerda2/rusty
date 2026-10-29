//! `Material.Set<Field>` / `Get<Field>` and the recipe keys for the table-driven
//! fields (the transparent surface, #1523; the map coordinates, #1573), through a
//! real Lua VM.

use std::cell::RefCell;

use mlua::Lua;

use crate::components::BlendMode;
use crate::scene::Scene;

/// Run `script` with the `Material` namespace over `scene`, returning its value.
fn eval<T: mlua::FromLuaMulti>(scene: &RefCell<Scene>, script: &str) -> mlua::Result<T> {
    let lua = Lua::new();
    lua.scope(|s| {
        super::super::register(&lua, s, scene).map_err(mlua::Error::RuntimeError)?;
        lua.load(script).eval()
    })
}

#[test]
fn every_field_sets_and_gets_through_lua() {
    let scene = RefCell::new(Scene::new());
    let id = scene.borrow_mut().add_entity("E".into());
    let (blend, shadows, preserve): (String, bool, bool) = eval(
        &scene,
        &format!(
            r#"Material.SetBlendMode({id}, "additive")
               Material.SetReceiveShadows({id}, false)
               return Material.GetBlendMode({id}), Material.GetReceiveShadows({id}),
                      Material.GetPreserveSpecular({id})"#
        ),
    )
    .unwrap();
    assert_eq!(
        (blend.as_str(), shadows, preserve),
        ("Additive", false, true)
    );
    let scene = scene.borrow();
    let key = scene.world.material(id).unwrap().first().to_string();
    assert_eq!(scene.materials[&key].blend_mode, BlendMode::Additive);
}

#[test]
fn a_bad_value_is_a_lua_error_naming_what_the_field_takes() {
    let scene = RefCell::new(Scene::new());
    let id = scene.borrow_mut().add_entity("E".into());
    let err = eval::<()>(&scene, &format!(r#"Material.SetBlendMode({id}, "glow")"#));
    assert!(err.unwrap_err().to_string().contains("Premultiply"));
    let err = eval::<()>(&scene, &format!(r#"Material.SetReceiveShadows({id}, 1)"#));
    assert!(err.unwrap_err().to_string().contains("receive_shadows"));
}

#[test]
fn a_getter_on_an_entity_without_a_material_is_nil() {
    let scene = RefCell::new(Scene::new());
    let id = scene.borrow_mut().add_entity("E".into());
    let got: Option<String> = eval(&scene, &format!("return Material.GetBlendMode({id})")).unwrap();
    assert_eq!(got, None);
}

#[test]
fn a_recipe_carries_the_surface_keys_and_refuses_a_bad_one() {
    let scene = RefCell::new(Scene::new());
    eval::<()>(
        &scene,
        r#"Material.DefineAsset("water", { render_mode = "Transparent",
            blend_mode = "Premultiply", receive_shadows = false })"#,
    )
    .unwrap();
    let m = scene.borrow().materials["water"].clone();
    assert_eq!(m.blend_mode, BlendMode::Premultiply);
    assert!(!m.receive_shadows);
    let err = eval::<()>(&scene, r#"Material.DefineAsset("bad", { blend_mode = 3 })"#);
    assert!(err.unwrap_err().to_string().contains("blend_mode"));
    assert!(
        !scene.borrow().materials.contains_key("bad"),
        "nothing defined"
    );
}

#[test]
fn map_coordinates_take_pairs_by_position_or_name() {
    let scene = RefCell::new(Scene::new());
    let id = scene.borrow_mut().add_entity("E".into());
    let (x, y, mode): (f32, f32, String) = eval(
        &scene,
        &format!(
            r#"Material.SetTiling({id}, {{x = 4, y = 2}})
               Material.SetOffset({id}, {{0.5, 0.25}})
               Material.SetUvMapping({id}, "triplanar")
               local t = Material.GetTiling({id})
               return t[1], t[2], Material.GetUvMapping({id})"#
        ),
    )
    .unwrap();
    assert_eq!((x, y, mode.as_str()), (4.0, 2.0, "Triplanar"));
    let err = eval::<()>(&scene, &format!("Material.SetOffset({id}, {{1}})"));
    assert!(err.unwrap_err().to_string().contains("offset"));
    eval::<()>(
        &scene,
        r#"Material.DefineAsset("floor", { tiling = {x = 8, y = 8}, offset = {1, 0},
            uv_mapping = "Planar", world_size = 2 })"#,
    )
    .unwrap();
    let c = scene.borrow().materials["floor"].coordinates.clone();
    assert_eq!(
        (c.tiling, c.offset, c.world_size),
        ([8.0; 2], [1.0, 0.0], 2.0)
    );
}
