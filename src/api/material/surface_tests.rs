//! `Material.Set<Field>` / `Get<Field>` and the recipe keys for the transparent-
//! surface fields (#1523), through a real Lua VM.

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
