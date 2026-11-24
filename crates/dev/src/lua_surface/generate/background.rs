use mlua::Lua;

use crate::bake::generate_lighting::refuse_while_baking;
use rusty_edit::LIGHTING_IN_PLAY;
use rusty_scene::lighting::bake_status::BakeStatus;
use rusty_script::api::{global_table, put, ApiCtx, Reg};

pub const NO_BAKE_HOST: &str = "Lighting.GenerateAsync: no editor frame loop here to \
run the bake in the background (a headless session or a standalone run); call \
Lighting.Generate() instead, which bakes in the one call";

pub(super) fn register(lua: &Lua, ctx: &ApiCtx) -> Reg {
    let (scene, scene_path, app) = (ctx.scene, ctx.scene_path, ctx.application);
    let table = global_table(lua, "Lighting")?;
    put(
        &table,
        "GenerateAsync",
        lua.create_function(move |lua, ()| {
            let refuse = |why: &str| Err(mlua::Error::runtime(why.to_string()));
            let app = app.get(lua)?;
            if !app.borrow().bake_host() {
                return refuse(NO_BAKE_HOST);
            }
            let scene = scene.get(lua)?;
            let mut scene = scene.borrow_mut();
            // Play asked for but not yet taken counts, as Unity's `isPlayingOrWillChangePlaymode`.
            if scene.is_playing() || app.borrow().play_heading() {
                return refuse(LIGHTING_IN_PLAY);
            }
            refuse_while_baking(&scene).map_err(mlua::Error::runtime)?;
            if scene_path.get(lua)?.borrow().is_none() {
                return refuse("Lighting.GenerateAsync: save the scene first");
            }
            scene.lighting_bake_requested = true;
            scene.lighting_bake_status = BakeStatus::Requested;
            // The editor sleeps outside Play: wake it for a frame to take the request.
            rusty_core::wake::frame();
            Ok(true)
        }),
    )?;
    put(
        &table,
        "IsRunning",
        lua.create_function(move |lua, ()| Ok(scene.get(lua)?.borrow().bake_busy())),
    )?;
    put(
        &table,
        "GetBakeStatus",
        lua.create_function(move |lua, ()| {
            let scene = scene.get(lua)?;
            let scene = scene.borrow();
            let status = &scene.lighting_bake_status;
            let out = lua.create_table()?;
            out.set("state", status.state())?;
            match &scene.lighting_queue {
                Some(spot) => out.set("message", spot.line(status))?,
                None => out.set("message", status.message())?,
            }
            Ok(out)
        }),
    )
}

#[cfg(test)]
#[path = "background_tests.rs"]
mod tests;
