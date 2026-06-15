//! src/api/material.rs — `Material` namespace.
//!
//! `SetMetallic/Roughness`, their map variants, and `SetTexture` over an
//! entity's optional texture component.

use std::cell::RefCell;
use std::rc::Rc;

use mlua::Lua;

use super::{put, Reg};
use crate::core::scene::Scene;

/// Register the `Material` namespace onto `lua`.
pub fn register(lua: &Lua, scene: &Rc<RefCell<Scene>>) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;

    let s = Rc::clone(scene);
    put(
        &table,
        "SetMetallic",
        lua.create_function(move |_, (id, val): (u32, f32)| {
            let mut scene = s.borrow_mut();
            if let Some(mut e) = scene.get_entity_mut(id) {
                if let Some(tex) = &mut e.texture {
                    tex.metallic = val;
                }
            }
            Ok(())
        }),
    )?;

    let s = Rc::clone(scene);
    put(
        &table,
        "SetRoughness",
        lua.create_function(move |_, (id, val): (u32, f32)| {
            let mut scene = s.borrow_mut();
            if let Some(mut e) = scene.get_entity_mut(id) {
                if let Some(tex) = &mut e.texture {
                    tex.roughness = val;
                }
            }
            Ok(())
        }),
    )?;

    let s = Rc::clone(scene);
    put(
        &table,
        "SetMetallicMap",
        lua.create_function(move |_, (id, path): (u32, String)| {
            let mut scene = s.borrow_mut();
            if let Some(mut e) = scene.get_entity_mut(id) {
                if let Some(tex) = &mut e.texture {
                    tex.metallic_map = Some(path);
                }
            }
            Ok(())
        }),
    )?;

    let s = Rc::clone(scene);
    put(
        &table,
        "SetRoughnessMap",
        lua.create_function(move |_, (id, path): (u32, String)| {
            let mut scene = s.borrow_mut();
            if let Some(mut e) = scene.get_entity_mut(id) {
                if let Some(tex) = &mut e.texture {
                    tex.roughness_map = Some(path);
                }
            }
            Ok(())
        }),
    )?;

    let s = Rc::clone(scene);
    put(
        &table,
        "SetTexture",
        lua.create_function(move |_, (id, path): (u32, String)| {
            let mut scene = s.borrow_mut();
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
