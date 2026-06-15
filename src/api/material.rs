//! src/api/material.rs — `Material` namespace.
//!
//! `SetMetallic/Roughness`, their map variants, and `SetTexture` over an
//! entity's optional texture component. Scoped callbacks over a borrowed
//! `&RefCell<&mut Scene>` (#57).

use std::cell::RefCell;

use mlua::{Lua, Scope};

use super::{put, Reg};
use crate::scene::Scene;

/// Register the `Material` namespace onto `lua`.
pub fn register<'a, 'scope>(
    scope: &Scope<'a, 'scope>,
    lua: &'a Lua,
    scene: &'scope RefCell<&'scope mut Scene>,
) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;

    put(
        &table,
        "SetMetallic",
        scope.create_function(move |_, (id, val): (u32, f32)| {
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
        &table,
        "SetRoughness",
        scope.create_function(move |_, (id, val): (u32, f32)| {
            let mut scene = scene.borrow_mut();
            if let Some(mut e) = scene.get_entity_mut(id) {
                if let Some(tex) = &mut e.texture {
                    tex.roughness = val;
                }
            }
            Ok(())
        }),
    )?;

    put(
        &table,
        "SetMetallicMap",
        scope.create_function(move |_, (id, path): (u32, String)| {
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
        &table,
        "SetRoughnessMap",
        scope.create_function(move |_, (id, path): (u32, String)| {
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
        &table,
        "SetTexture",
        scope.create_function(move |_, (id, path): (u32, String)| {
            let mut scene = scene.borrow_mut();
            if let Some(mut e) = scene.get_entity_mut(id) {
                if let Some(tex) = &mut e.texture {
                    tex.path = path;
                }
            }
            Ok(())
        }),
    )?;

    lua.globals()
        .set("Material", table)
        .map_err(|e| e.to_string())
}
