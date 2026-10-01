//! src/api/rect_mask.rs — `RectMask` namespace (#418).
//!
//! Get/Set over an entity's `RectMaskComponent` (Unity's `RectMask2D`): the clip's
//! `Padding` inset and its soft-edge `Feather` (#428). Whether a mask exists is `Scene.AddComponent(id, "RectMask")` /
//! `RemoveComponent`. The setters route through the shared
//! `scene::authoring::rect_mask` op the inspector card uses.

use std::cell::RefCell;

use glam::Vec4;
use mlua::Lua;

use super::{put, Reg};
use crate::scene::authoring::rect_mask as mask_ops;
use crate::scene::Scene;

/// Register the `RectMask` namespace onto `lua`.
pub fn register<'lua, 'scope>(
    lua: &'lua Lua,
    scope: &mlua::Scope<'lua, 'scope>,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;
    put(
        &table,
        "GetPadding",
        scope.create_function(|_, id: u32| {
            let scene = scene.borrow();
            let p = scene.world.rect_mask(id).map_or(Vec4::ZERO, |m| m.padding);
            Ok((p.x, p.y, p.z, p.w))
        }),
    )?;
    put(
        &table,
        "SetPadding",
        scope.create_function(|_, (id, l, b, r, t): (u32, f32, f32, f32, f32)| {
            if let Some(mut m) = scene.borrow_mut().world.rect_mask_mut(id) {
                mask_ops::set_padding(&mut m, Vec4::new(l, b, r, t));
            }
            Ok(())
        }),
    )?;
    put(
        &table,
        "GetFeather",
        scope.create_function(|_, id: u32| {
            let scene = scene.borrow();
            Ok(scene.world.rect_mask(id).map_or(0.0, |m| m.feather))
        }),
    )?;
    put(
        &table,
        "SetFeather",
        scope.create_function(|_, (id, feather): (u32, f32)| {
            if let Some(mut m) = scene.borrow_mut().world.rect_mask_mut(id) {
                mask_ops::set_feather(&mut m, feather);
            }
            Ok(())
        }),
    )?;
    lua.globals()
        .set("RectMask", table)
        .map_err(|e| e.to_string())
}
