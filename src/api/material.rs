//! src/api/material.rs — `Material` namespace.
//!
//! `SetMetallic/Roughness`, their map variants, and `SetTexture` over an
//! entity's optional texture component.

use std::cell::RefCell;

use mlua::Lua;

use super::{put, Reg};
use crate::scene::Scene;

/// Register the `Material` namespace onto `lua`.
pub fn register<'lua, 'scope>(
    lua: &'lua Lua,
    scope: &mlua::Scope<'lua, 'scope>,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;

    register_scalars(scope, &table, scene)?;
    register_maps(scope, &table, scene)?;

    lua.globals()
        .set("Material", table)
        .map_err(|e| e.to_string())
}

/// Scalar PBR knobs: `SetMetallic` / `SetRoughness`.
fn register_scalars<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    put(
        table,
        "SetMetallic",
        scope.create_function(|_, (id, val): (u32, f32)| {
            let mut scene = scene.borrow_mut();
            if let Some(mut e) = scene.get_entity_mut(id) {
                if let Some(tex) = &mut e.texture {
                    tex.metallic = val;
                }
            }
            Ok(())
        }),
    )?;

    put(
        table,
        "SetRoughness",
        scope.create_function(|_, (id, val): (u32, f32)| {
            let mut scene = scene.borrow_mut();
            if let Some(mut e) = scene.get_entity_mut(id) {
                if let Some(tex) = &mut e.texture {
                    tex.roughness = val;
                }
            }
            Ok(())
        }),
    )
}

/// Texture-map paths: `SetMetallicMap` / `SetRoughnessMap` / `SetTexture`.
fn register_maps<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    put(
        table,
        "SetMetallicMap",
        scope.create_function(|_, (id, path): (u32, String)| {
            let mut scene = scene.borrow_mut();
            if let Some(mut e) = scene.get_entity_mut(id) {
                if let Some(tex) = &mut e.texture {
                    tex.metallic_map = Some(path);
                }
            }
            Ok(())
        }),
    )?;

    put(
        table,
        "SetRoughnessMap",
        scope.create_function(|_, (id, path): (u32, String)| {
            let mut scene = scene.borrow_mut();
            if let Some(mut e) = scene.get_entity_mut(id) {
                if let Some(tex) = &mut e.texture {
                    tex.roughness_map = Some(path);
                }
            }
            Ok(())
        }),
    )?;

    put(
        table,
        "SetTexture",
        scope.create_function(|_, (id, path): (u32, String)| {
            let mut scene = scene.borrow_mut();
            if let Some(mut e) = scene.get_entity_mut(id) {
                if let Some(tex) = &mut e.texture {
                    tex.path = path;
                }
            }
            Ok(())
        }),
    )
}
