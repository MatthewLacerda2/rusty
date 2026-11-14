//! crates/script/src/api/mask.rs — `Mask` namespace (#428).
//!
//! Get/Set over an entity's `MaskComponent` (Unity's `Mask`): whether the mask's
//! own graphic also draws. Whether a mask exists is `Scene.AddComponent(id,
//! "Mask")` / `RemoveComponent`. The setter routes through the shared
//! `scene::authoring::ui::rect_mask` op the inspector card uses.

use mlua::Lua;

use super::{put, Reg};
use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use rusty_scene::authoring::ui::rect_mask as mask_ops;
use rusty_scene::Scene;

/// Register the `Mask` namespace onto `lua`.
pub fn register(lua: &Lua, scene: ApiCell<Scene>) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;
    put(
        &table,
        "GetShowMaskGraphic",
        lua.create_function(move |lua, id: EntityArg| {
            let scene = &*scene.get(lua)?;
            let scene = scene.borrow();
            Ok(scene
                .world
                .mask(id.read())
                .is_some_and(|m| m.show_mask_graphic))
        }),
    )?;
    put(
        &table,
        "SetShowMaskGraphic",
        lua.create_function(move |lua, (id, show): (EntityArg, bool)| {
            let id = id.live("Mask", "SetShowMaskGraphic")?;
            if let Some(mut m) = scene.get(lua)?.borrow_mut().world.mask_mut(id) {
                mask_ops::set_show_mask_graphic(&mut m, show);
            }
            Ok(())
        }),
    )?;
    lua.globals().set("Mask", table).map_err(|e| e.to_string())
}
