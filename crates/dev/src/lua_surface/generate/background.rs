//! `Lighting.GenerateAsync()`, `Lighting.IsRunning()` and `Lighting.GetBakeStatus()`
//! (#1999), Unity's `Lightmapping.BakeAsync` and `Lightmapping.isRunning`: the editor's Generate
//! Lighting button from a script. `Lighting.Generate()` blocks its caller for the
//! whole bake, and an MCP client gives up on a tool call that has said nothing for
//! 30 minutes; this starts the button's own job and returns at once, so an agent
//! polls `IsRunning()` and reads `Lighting.GetBakeReport(kind)` when it is done.
//! `GetBakeStatus()` says how the last one ended (done, cancelled, failed or
//! refused, with its line), in band: a failure in the editor's job reaches only
//! the window's console otherwise, and `IsRunning()` alone reads `false` either way.
//!
//! The call only *requests* the bake (`Scene::lighting_bake_requested`): the
//! editor's inspector takes it on its next frame (`bake::poll`) and runs the
//! button's job, so nothing a script starts differs from what the button starts.
//! It is refused where the button is (Play, a bake already running, an unsaved
//! scene) and in a host with no frame loop to step the job (a headless session),
//! which names `Lighting.Generate()` instead. Play counts from the moment it is
//! requested (`Application::play_heading`); the other way round, Play is refused
//! while a request waits or the job runs (`Scene::play_refusal`, #2042). A request
//! belongs to the scene it was made in: File ▸ New and a load drop it
//! (`Scene::drop_bake_request`).

use mlua::Lua;

use crate::bake::generate_lighting::refuse_while_baking;
use rusty_scene::lighting::bake_status::BakeStatus;
use rusty_scene::LIGHTING_IN_PLAY;
use rusty_script::api::{global_table, put, ApiCtx, Reg};

/// Why a host that steps no background bake refuses `GenerateAsync`.
pub const NO_BAKE_HOST: &str = "Lighting.GenerateAsync: no editor frame loop here to \
run the bake in the background (a headless session or a standalone run); call \
Lighting.Generate() instead, which bakes in the one call";

/// Add `GenerateAsync`, `IsRunning` and `GetBakeStatus` to the `Lighting` table.
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
            // Play asked for but not yet taken counts, as it does for the button
            // (Unity's `isPlayingOrWillChangePlaymode`).
            if scene.is_playing() || app.borrow().play_heading() {
                return refuse(LIGHTING_IN_PLAY);
            }
            refuse_while_baking(&scene).map_err(mlua::Error::runtime)?;
            if scene_path.get(lua)?.borrow().is_none() {
                return refuse("Lighting.GenerateAsync: save the scene first");
            }
            scene.lighting_bake_requested = true;
            scene.lighting_bake_status = BakeStatus::Requested;
            // The editor sleeps outside Play (#1721): a frame takes the request.
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
            let status = scene.get(lua)?.borrow().lighting_bake_status.clone();
            let out = lua.create_table()?;
            out.set("state", status.state())?;
            out.set("message", status.message())?;
            Ok(out)
        }),
    )
}

#[cfg(test)]
#[path = "background_tests.rs"]
mod tests;
