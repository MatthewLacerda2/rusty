mod graph;
mod ik;
mod layers;
mod montage;
mod parameters;
mod play;
mod root_motion;
mod speed;

use mlua::Lua;

use super::{put, Reg};
use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use crate::scripting::ConsoleLogs;
use rusty_edit::authoring::animator as animator_ops;
use rusty_scene::Scene;

pub fn register(lua: &Lua, scene: ApiCell<Scene>, console: ApiCell<ConsoleLogs>) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;

    play::register(lua, &table, scene, console)?;
    register_stop(lua, &table, scene)?;
    register_pause_resume(lua, &table, scene)?;
    register_set_looping(lua, &table, scene)?;
    speed::register(lua, &table, scene)?;
    root_motion::register(lua, &table, scene)?;
    parameters::register(lua, &table, scene, console)?;
    register_get_bone(lua, &table, scene)?;
    graph::register(lua, &table, scene, console)?;
    layers::register(lua, &table, scene, console)?;
    ik::register(lua, &table, scene, console)?;
    montage::register(lua, &table, scene, console)?;

    lua.globals()
        .set("Animator", table)
        .map_err(|e| e.to_string())
}

fn register_get_bone(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "GetBone",
        lua.create_function(move |lua, (id, name): (EntityArg, String)| {
            Ok(scene.get(lua)?.borrow().find_bone(id.read(), &name))
        }),
    )?;
    put(
        table,
        "GetClip",
        lua.create_function(move |lua, id: EntityArg| {
            let scene = &*scene.get(lua)?;
            let scene = scene.borrow();
            Ok(scene
                .world
                .animator(id.read())
                .map(|a| a.base.current_clip.clone()))
        }),
    )
}

fn register_stop(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "Stop",
        lua.create_function(move |lua, id: EntityArg| {
            let scene = &*scene.get(lua)?;
            let id = id.live("Animator", "Stop")?;
            let mut scene = scene.borrow_mut();
            if let Some(mut c) = scene.world.animator_mut(id) {
                animator_ops::set_playing(&mut c, false);
            }
            Ok(())
        }),
    )
}

fn register_pause_resume(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "Pause",
        lua.create_function(move |lua, id: EntityArg| {
            let scene = &*scene.get(lua)?;
            let id = id.live("Animator", "Pause")?;
            let mut scene = scene.borrow_mut();
            if let Some(mut c) = scene.world.animator_mut(id) {
                animator_ops::set_freeze(&mut c, true);
            }
            Ok(())
        }),
    )?;

    put(
        table,
        "Resume",
        lua.create_function(move |lua, id: EntityArg| {
            let scene = &*scene.get(lua)?;
            let id = id.live("Animator", "Resume")?;
            let mut scene = scene.borrow_mut();
            if let Some(mut c) = scene.world.animator_mut(id) {
                animator_ops::set_freeze(&mut c, false);
            }
            Ok(())
        }),
    )
}

fn register_set_looping(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "SetLooping",
        lua.create_function(move |lua, (id, looping): (EntityArg, bool)| {
            let scene = &*scene.get(lua)?;
            let id = id.live("Animator", "SetLooping")?;
            let mut scene = scene.borrow_mut();
            if let Some(mut c) = scene.world.animator_mut(id) {
                animator_ops::set_looping(&mut c, looping);
            }
            Ok(())
        }),
    )
}
