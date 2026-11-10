//! `Lighting.GenerateAsync()` and `Lighting.IsRunning()` (#1999), Unity's
//! `Lightmapping.BakeAsync` and `Lightmapping.isRunning`: the editor's Generate
//! Lighting button from a script. `Lighting.Generate()` blocks its caller for the
//! whole bake, and an MCP client gives up on a tool call that has said nothing for
//! 30 minutes; this starts the button's own job and returns at once, so an agent
//! polls `IsRunning()` and reads `Lighting.GetBakeReport(kind)` when it is done.
//!
//! The call only *requests* the bake (`Scene::lighting_bake_requested`): the
//! editor's inspector takes it on its next frame (`bake::poll`) and runs the
//! button's job, so nothing a script starts differs from what the button starts.
//! It is refused where the button is (Play, a bake already running, an unsaved
//! scene) and in a host with no frame loop to step the job (a headless session),
//! which names `Lighting.Generate()` instead.

use mlua::Lua;

use crate::api::{global_table, put, ApiCtx, Reg};
use crate::dev::bake::generate_lighting::refuse_while_baking;
use crate::scene::LIGHTING_IN_PLAY;

/// Why a host that steps no background bake refuses `GenerateAsync`.
pub const NO_BAKE_HOST: &str = "Lighting.GenerateAsync: no editor frame loop here to \
run the bake in the background (a headless session or a standalone run); call \
Lighting.Generate() instead, which bakes in the one call";

/// Add `GenerateAsync` and `IsRunning` to the `Lighting` table.
pub(super) fn register(lua: &Lua, ctx: &ApiCtx) -> Reg {
    let (scene, scene_path, app) = (ctx.scene, ctx.scene_path, ctx.application);
    let table = global_table(lua, "Lighting")?;
    put(
        &table,
        "GenerateAsync",
        lua.create_function(move |lua, ()| {
            let refuse = |why: &str| Err(mlua::Error::runtime(why.to_string()));
            if !app.get(lua)?.borrow().bake_host() {
                return refuse(NO_BAKE_HOST);
            }
            let scene = scene.get(lua)?;
            let mut scene = scene.borrow_mut();
            if scene.is_playing() {
                return refuse(LIGHTING_IN_PLAY);
            }
            refuse_while_baking(&scene).map_err(mlua::Error::runtime)?;
            if scene_path.get(lua)?.borrow().is_none() {
                return refuse("Lighting.GenerateAsync: save the scene first");
            }
            scene.lighting_bake_requested = true;
            // The editor sleeps outside Play (#1721): a frame takes the request.
            crate::core::wake::frame();
            Ok(true)
        }),
    )?;
    put(
        &table,
        "IsRunning",
        lua.create_function(move |lua, ()| {
            let scene = scene.get(lua)?;
            let scene = scene.borrow();
            Ok(scene.lighting_bake_running || scene.lighting_bake_requested)
        }),
    )
}

#[cfg(test)]
#[path = "background_tests.rs"]
mod tests;
