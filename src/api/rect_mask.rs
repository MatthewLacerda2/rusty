//! src/api/rect_mask.rs — `RectMask` namespace (#418).
//!
//! Get/Set over an entity's `RectMaskComponent` (Unity's `RectMask2D`): the clip's
//! `Padding` inset and its soft-edge `Feather` (#428). Whether a mask exists is `Scene.AddComponent(id, "RectMask")` /
//! `RemoveComponent`. The setters route through the shared
//! `scene::authoring::ui::rect_mask` op the inspector card uses.

use glam::Vec4;
use mlua::Lua;

use super::{put, Reg};
use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use crate::scene::authoring::ui::rect_mask as mask_ops;
use crate::scene::Scene;

/// Register the `RectMask` namespace onto `lua`.
pub fn register(lua: &Lua, scene: ApiCell<Scene>) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;
    put(
        &table,
        "GetPadding",
        lua.create_function(move |lua, id: EntityArg| {
            let scene = &*scene.get(lua)?;
            let scene = scene.borrow();
            let p = scene
                .world
                .rect_mask(id.read())
                .map_or(Vec4::ZERO, |m| m.padding);
            Ok((p.x, p.y, p.z, p.w))
        }),
    )?;
    put(
        &table,
        "SetPadding",
        lua.create_function(
            move |lua, (id, l, b, r, t): (EntityArg, f32, f32, f32, f32)| {
                let id = id.live("RectMask", "SetPadding")?;
                if let Some(mut m) = scene.get(lua)?.borrow_mut().world.rect_mask_mut(id) {
                    mask_ops::set_padding(&mut m, Vec4::new(l, b, r, t));
                }
                Ok(())
            },
        ),
    )?;
    put(
        &table,
        "GetFeather",
        lua.create_function(move |lua, id: EntityArg| {
            let scene = &*scene.get(lua)?;
            let scene = scene.borrow();
            Ok(scene.world.rect_mask(id.read()).map_or(0.0, |m| m.feather))
        }),
    )?;
    put(
        &table,
        "SetFeather",
        lua.create_function(move |lua, (id, feather): (EntityArg, f32)| {
            let id = id.live("RectMask", "SetFeather")?;
            if let Some(mut m) = scene.get(lua)?.borrow_mut().world.rect_mask_mut(id) {
                mask_ops::set_feather(&mut m, feather);
            }
            Ok(())
        }),
    )?;
    lua.globals()
        .set("RectMask", table)
        .map_err(|e| e.to_string())
}
