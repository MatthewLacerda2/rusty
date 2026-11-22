//! The Lua `Material.*` surface and the shared `authoring::material` ops converge.

use std::cell::RefCell;
use std::rc::Rc;

use mlua::Lua;
use rusty::edit::authoring::material as mat_ops;
use rusty::scene::{RenderMode, Scene};
use rusty::script::api::bind;

#[test]
fn lua_material_api_and_shared_op_converge() -> Result<(), Box<dyn std::error::Error>> {
    // Lua `Material.*` and the editor card route through the same `authoring::material::*` ops: the
    // same change either way must yield equal `MaterialAsset` state; the
    // `SetAlpha`/`SetAlphaCutoff` clamp (2.0 / -1.0 -> 1.0 / 0.0) proves validation is
    // single-sourced.
    let scene = Rc::new(RefCell::new(Scene::new()));
    let via_lua = scene.borrow_mut().add_entity("ViaLua".to_string());
    let via_op = scene.borrow_mut().add_entity("ViaOp".to_string());

    let lua = Lua::new();
    rusty::script::api::material::register(&lua, bind(&lua, &scene)).unwrap();
    lua.load(format!(
        r#"
        Material.SetMetallic({via_lua}, 0.7)
        Material.SetRoughness({via_lua}, 0.2)
        Material.SetEmissive({via_lua}, {{1.0, 0.5, 0.0}})
        Material.SetColor({via_lua}, {{0.25, 2.0, 0.5}})
        Material.SetTexture({via_lua}, "albedo.png")
        Material.SetRenderMode({via_lua}, "transparent")
        Material.SetAlpha({via_lua}, 2.0)
        Material.SetAlphaCutoff({via_lua}, -1.0)
    "#
    ))
    .exec()
    .unwrap();

    {
        let mut sc = scene.borrow_mut();
        let key = mat_ops::ensure_material_key(&mut sc, via_op).unwrap();
        mat_ops::set_metallic(&mut sc.materials, &key, 0.7);
        mat_ops::set_roughness(&mut sc.materials, &key, 0.2);
        mat_ops::set_emissive(&mut sc.materials, &key, [1.0, 0.5, 0.0]);
        mat_ops::set_base_color(&mut sc.materials, &key, [0.25, 2.0, 0.5]);
        mat_ops::set_base_color_map(&mut sc.materials, &key, "albedo.png".to_string());
        mat_ops::set_render_mode(&mut sc.materials, &key, RenderMode::Transparent);
        mat_ops::set_alpha(&mut sc.materials, &key, 2.0);
        mat_ops::set_alpha_cutoff(&mut sc.materials, &key, -1.0);
    }

    let sc = scene.borrow();
    let lua_mat = sc.material_asset_of(via_lua).unwrap();
    let op_mat = sc.material_asset_of(via_op).unwrap();

    assert_eq!(lua_mat.metallic, op_mat.metallic);
    assert_eq!(lua_mat.roughness, op_mat.roughness);
    assert_eq!(lua_mat.emissive, op_mat.emissive);
    assert_eq!(lua_mat.base_color, [0.25, 1.0, 0.5], "clamped by the op");
    assert_eq!(lua_mat.base_color, op_mat.base_color);
    assert_eq!(lua_mat.base_color_map, op_mat.base_color_map);
    assert_eq!(lua_mat.render_mode, op_mat.render_mode);
    assert_eq!(lua_mat.alpha, 1.0);
    assert_eq!(op_mat.alpha, 1.0);
    assert_eq!(lua_mat.alpha_cutoff, 0.0);
    assert_eq!(op_mat.alpha_cutoff, 0.0);
    Ok(())
}
