//! The Lua `Material.*` surface and the shared `authoring::material` ops converge (#287).

use std::cell::RefCell;

use mlua::Lua;
use rusty::scene::authoring::material as mat_ops;
use rusty::scene::{RenderMode, Scene};

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
    let lua_mat = sc.material_asset_of(via_lua).unwrap();
    let op_mat = sc.material_asset_of(via_op).unwrap();

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
