//! src/api/mask.rs — `Mask` namespace (#428).
//!
//! Get/Set over an entity's `MaskComponent` (Unity's `Mask`): whether the mask's
//! own graphic also draws. Whether a mask exists is `Scene.AddComponent(id,
//! "Mask")` / `RemoveComponent`. The setter routes through the shared
//! `scene::authoring::rect_mask` op the inspector card uses.

use std::cell::RefCell;

use mlua::Lua;

use super::{put, Reg};
use crate::scene::authoring::rect_mask as mask_ops;
use crate::scene::Scene;

/// Register the `Mask` namespace onto `lua`.
pub fn register<'scope>(
    lua: &Lua,
    scope: &'scope mlua::Scope<'scope, '_>,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;
    put(
        &table,
        "GetShowMaskGraphic",
        scope.create_function(|_, id: u32| {
            let scene = scene.borrow();
            Ok(scene.world.mask(id).is_some_and(|m| m.show_mask_graphic))
        }),
    )?;
    put(
        &table,
        "SetShowMaskGraphic",
        scope.create_function(|_, (id, show): (u32, bool)| {
            if let Some(mut m) = scene.borrow_mut().world.mask_mut(id) {
                mask_ops::set_show_mask_graphic(&mut m, show);
            }
            Ok(())
        }),
    )?;
    lua.globals().set("Mask", table).map_err(|e| e.to_string())
}
