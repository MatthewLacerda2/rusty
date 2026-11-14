//! crates/script/src/api/animator/root_motion.rs — the `Animator` root-motion surface (#1745).
//!
//! Unity's `applyRootMotion` and `deltaPosition` / `deltaRotation`:
//! `SetApplyRootMotion` / `GetApplyRootMotion` toggle whether the root bone's
//! horizontal travel and yaw move the entity (through its CharacterController,
//! else its NavMeshAgent, else its Transform), `SetRootBone` / `GetRootBone`
//! name that bone (empty: the skeleton's first root joint), and
//! `GetRootMotion` reads the last fixed step's delta — world-space metres and a
//! yaw in degrees — whether or not it was applied, for a script that moves the
//! entity itself. Writes go through `authoring::animator`, the ops the
//! inspector's Animator card uses.

use mlua::IntoLuaMulti;

use super::{put, Reg};
use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use rusty_scene::authoring::animator as animator_ops;
use rusty_scene::Scene;

/// Register the root-motion verbs onto the `Animator` table.
pub(super) fn register(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    register_settings(lua, table, scene)?;
    register_get_root_motion(lua, table, scene)
}

/// The saved settings: `Set`/`GetApplyRootMotion` and `Set`/`GetRootBone`.
fn register_settings(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "SetApplyRootMotion",
        lua.create_function(move |lua, (id, apply): (EntityArg, bool)| {
            let scene = &*scene.get(lua)?;
            let id = id.live("Animator", "SetApplyRootMotion")?;
            if let Some(mut c) = scene.borrow_mut().world.animator_mut(id) {
                animator_ops::set_apply_root_motion(&mut c, apply);
            }
            Ok(())
        }),
    )?;
    put(
        table,
        "GetApplyRootMotion",
        lua.create_function(move |lua, id: EntityArg| {
            let scene = &*scene.get(lua)?;
            let scene = scene.borrow();
            Ok(scene.world.animator(id.read()).map(|a| a.apply_root_motion))
        }),
    )?;
    put(
        table,
        "SetRootBone",
        lua.create_function(move |lua, (id, bone): (EntityArg, String)| {
            let scene = &*scene.get(lua)?;
            let id = id.live("Animator", "SetRootBone")?;
            if let Some(mut c) = scene.borrow_mut().world.animator_mut(id) {
                animator_ops::set_root_bone(&mut c, bone);
            }
            Ok(())
        }),
    )?;
    put(
        table,
        "GetRootBone",
        lua.create_function(move |lua, id: EntityArg| {
            let scene = &*scene.get(lua)?;
            let scene = scene.borrow();
            Ok(scene.world.animator(id.read()).map(|a| a.root_bone.clone()))
        }),
    )
}

/// `GetRootMotion`: the last step's world-space delta and yaw (degrees).
fn register_get_root_motion(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "GetRootMotion",
        lua.create_function(move |lua, id: EntityArg| {
            let scene = &*scene.get(lua)?;
            let scene = scene.borrow();
            match scene.world.animator(id.read()).map(|a| a.root_motion.last) {
                Some(m) => {
                    let p = m.position;
                    (p.x, p.y, p.z, m.yaw.to_degrees()).into_lua_multi(lua)
                }
                None => Ok(mlua::MultiValue::new()),
            }
        }),
    )
}
