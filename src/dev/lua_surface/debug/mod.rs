//! src/dev/lua_surface/debug/ — `Debug` namespace (DEV-ONLY).
//!
//! `Debug.Log/Warn/Error`, the demo index's `Debug.Mark` (#1597), the structured scene-read `Debug.Snapshot` /
//! `Debug.SnapshotEntity` (#180), the headless `Debug.Preview` asset shot (#353) and its
//! named-material twin `Debug.PreviewMaterial` (#404),
//! the frame stats `Debug.Stats` (#433), and the audio reads `Debug.AudioEvents` /
//! `Debug.AudioVoices` (`audio.rs`, #1247), and the GPU readback
//! `Debug.GetAdaptedExposure` (#1444), and the debug lines `Debug.DrawLine` /
//! `Debug.DrawRay` (`draw.rs`, #1882) — the agent's observation channels:
//! structured, visual, performance and aural. Registered
//! only in dev builds — stripped from the shipped game, like Unity `Debug.*` under
//! `[Conditional]`: it lives in `dev`, which installs it onto the surface (#737).

mod audio;
mod draw;

use mlua::{Lua, Table};

use crate::dev::preview::{capture_asset, capture_material, PreviewOptions};
use crate::preview::PreviewMesh;
use crate::scene::game_camera_from_scene;
use crate::script::api::{put, snapshot, ApiCtx, Reg};
use crate::script::scripting::Mark;

/// Register the `Debug` namespace onto `lua` (dev builds only).
pub fn register(lua: &Lua, ctx: &ApiCtx) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;

    register_logging(lua, &table, ctx.console)?;
    register_snapshot(lua, &table, ctx)?;
    register_preview(lua, &table)?;
    register_preview_material(lua, &table, ctx.scene)?;
    register_stats(lua, &table, ctx.stats)?;
    register_adapted_exposure(lua, &table, ctx.scene)?;
    audio::register(lua, &table, ctx)?;
    draw::register(lua, &table, ctx.scene)?;

    lua.globals().set("Debug", table).map_err(|e| e.to_string())
}

/// `Debug.Log/Warn/Error` — append to the shared console buffer.
fn register_logging(
    lua: &mlua::Lua,
    table: &mlua::Table,
    console: crate::script::api::ApiCell<crate::script::scripting::ConsoleLogs>,
) -> Reg {
    put(
        table,
        "Log",
        lua.create_function(move |lua, msg: String| {
            console.get(lua)?.borrow_mut().info(msg);
            Ok(())
        }),
    )?;
    put(
        table,
        "Warn",
        lua.create_function(move |lua, msg: String| {
            console.get(lua)?.borrow_mut().warn(msg);
            Ok(())
        }),
    )?;
    put(
        table,
        "Error",
        lua.create_function(move |lua, msg: String| {
            console.get(lua)?.borrow_mut().error(msg);
            Ok(())
        }),
    )?;
    // `Debug.Mark(label [, value])` — a game fact for the recording demo's index
    // (#1597); a no-op while no demo records.
    put(
        table,
        "Mark",
        lua.create_function(move |lua, (label, value): (String, mlua::Value)| {
            let value = match value {
                mlua::Value::Nil => None,
                value => Some(crate::dev::demo::plain_value(&value).map_err(|kind| {
                    let msg = format!("Debug.Mark: a value must be plain data, not a {kind}");
                    mlua::Error::RuntimeError(msg)
                })?),
            };
            console.get(lua)?.borrow_mut().mark(Mark { label, value });
            Ok(())
        }),
    )
}

/// `Debug.Stats()` — the frame stats (#433) as a table: `frames`, each metric's
/// `{last, min, avg, max, samples}`, and `systems` (per-system CPU ms). Filled by the
/// dev layer's schedule probe; empty (`frames = 0`) where none is installed.
fn register_stats(
    lua: &mlua::Lua,
    table: &Table,
    stats: crate::script::api::ApiCell<crate::core::frame_stats::FrameStats>,
) -> Reg {
    put(
        table,
        "Stats",
        lua.create_function(move |lua, ()| {
            stats.with(lua, |stats| crate::dev::stats::to_lua(lua, &stats.borrow()))
        }),
    )
}

/// `Debug.GetAdaptedExposure()` — the automatic exposure the screen last applied, in
/// EV, read back from the GPU (#1444); `nil` before an `Automatic` frame has metered.
/// Dev-only on purpose: a GPU readback is no sim input, so gameplay's
/// `Graphics.GetExposure` returns the configured EV instead.
fn register_adapted_exposure(
    lua: &mlua::Lua,
    table: &Table,
    scene: crate::script::api::ApiCell<crate::scene::Scene>,
) -> Reg {
    put(
        table,
        "GetAdaptedExposure",
        lua.create_function(move |lua, ()| Ok(scene.get(lua)?.borrow().screen_exposure)),
    )
}

/// `Debug.Preview(asset_path, out_png [, opts])` — render an asset's preview to a PNG
/// headlessly and return the written path, so it chains straight off an authoring bake:
/// `Debug.Preview(Shader.Bake(recipe), "out/preview.png")` (#353).
///
/// Returns the path on success and `nil` when the box has no GPU/software adapter (a
/// skip, not a failure — the harness must still run on a GPU-less CI box). A bad asset
/// path or an unpreviewable kind raises, so a typo can't masquerade as a blank render.
/// Borrows no engine state — it builds its own isolated scene and its own headless
/// renderer — so it registers as a plain static function like `Shader`/`Texture`.
fn register_preview(lua: &Lua, table: &Table) -> Reg {
    put(
        table,
        "Preview",
        lua.create_function(
            |_, (asset_path, out_png, opts): (String, String, Option<Table>)| {
                let options = preview_options(opts.as_ref())?;
                let written = capture_asset(&asset_path, &out_png, options)
                    .map_err(mlua::Error::RuntimeError)?;
                Ok(written.then_some(out_png))
            },
        ),
    )
}

/// `Debug.PreviewMaterial(name, out_png [, opts])` — the material-asset arm of
/// `Debug.Preview` (#404). Material assets live in the scene's library, not on disk,
/// so this one borrows the scene to look `name` up (a name it doesn't hold raises),
/// then renders the copy in the same isolated preview scene as the Inspector's
/// Material card. Same options and the same `nil`-without-a-GPU return.
fn register_preview_material(
    lua: &mlua::Lua,
    table: &Table,
    scene: crate::script::api::ApiCell<crate::scene::Scene>,
) -> Reg {
    put(
        table,
        "PreviewMaterial",
        lua.create_function(
            move |lua, (name, out_png, opts): (String, String, Option<Table>)| {
                let options = preview_options(opts.as_ref())?;
                let written = capture_material(
                    &scene.get(lua)?.borrow().materials,
                    &name,
                    &out_png,
                    options,
                )
                .map_err(mlua::Error::RuntimeError)?;
                Ok(written.then_some(out_png))
            },
        ),
    )
}

/// Read the optional `{ mesh = "sphere"|"cube"|"suzanne", resolution = 512 }` table,
/// defaulting each field independently. An unknown mesh name raises rather than
/// silently falling back, so a misspelled `"spere"` is caught at the call, not by the
/// agent squinting at the wrong picture.
fn preview_options(opts: Option<&Table>) -> mlua::Result<PreviewOptions> {
    let defaults = PreviewOptions::default();
    let Some(opts) = opts else {
        return Ok(defaults);
    };
    let mesh = match opts.get::<Option<String>>("mesh")? {
        Some(name) => parse_mesh(&name)?,
        None => defaults.mesh,
    };
    let resolution = opts
        .get::<Option<u32>>("resolution")?
        .unwrap_or(defaults.resolution);
    Ok(PreviewOptions { mesh, resolution })
}

/// Map a Lua mesh name onto [`PreviewMesh`], matching the editor toggle's three
/// choices case-insensitively.
fn parse_mesh(name: &str) -> mlua::Result<PreviewMesh> {
    match name.to_lowercase().as_str() {
        "sphere" => Ok(PreviewMesh::Sphere),
        "cube" => Ok(PreviewMesh::Cube),
        "suzanne" => Ok(PreviewMesh::Suzanne),
        other => Err(mlua::Error::RuntimeError(format!(
            "unknown preview mesh {other:?} — expected \"sphere\", \"cube\" or \"suzanne\""
        ))),
    }
}

/// `Debug.Snapshot([opts])` (whole world) and `Debug.SnapshotEntity(id)` (one entity) —
/// the structured scene-read, returned as a pretty JSON string the agent parses.
fn register_snapshot(lua: &mlua::Lua, table: &mlua::Table, ctx: &ApiCtx) -> Reg {
    let scene = ctx.scene;
    let camera = ctx.camera;
    let time = ctx.time;
    let (screen, video) = (ctx.screen, ctx.video);
    // The main view (#1619): the camera entity's pose and lens when the scene has one.
    let main = move |lua: &mlua::Lua| -> mlua::Result<_> {
        let (camera, scene) = (camera.get(lua)?, scene.get(lua)?);
        let main = game_camera_from_scene(&camera.borrow(), &scene.borrow());
        Ok(main)
    };
    let view = move |lua: &mlua::Lua| -> mlua::Result<_> {
        let (screen, video) = (screen.get(lua)?, video.get(lua)?);
        let view = screen.borrow().view(&video.borrow(), Some(main(lua)?));
        Ok(view)
    };
    put(
        table,
        "Snapshot",
        lua.create_function(move |lua, opts: Option<Table>| {
            let scene = &*scene.get(lua)?;
            // `{ bones = true }` includes skeleton bones, left out by default (#453).
            let bones = opts.and_then(|o| o.get::<Option<bool>>("bones").ok().flatten());
            let value = snapshot::world_value_with(
                &scene.borrow(),
                &main(lua)?,
                time.get(lua)?.borrow().frame_count,
                scene.borrow().is_playing(),
                &view(lua)?,
                bones.unwrap_or(false),
            );
            Ok(serde_json::to_string_pretty(&value).unwrap_or_default())
        }),
    )?;

    let scene = ctx.scene;
    put(
        table,
        "SnapshotEntity",
        lua.create_function(move |lua, id: crate::script::api::entity_arg::EntityArg| {
            let scene = &*scene.get(lua)?;
            let id = id.read();
            let scene = scene.borrow();
            let world_matrix = scene.world.local_to_world(id);
            let value = if scene.world.contains(id) {
                snapshot::entity_value(&scene, id, world_matrix, &view(lua)?)
            } else {
                serde_json::Value::Null
            };
            Ok(serde_json::to_string_pretty(&value).unwrap_or_default())
        }),
    )
}

#[cfg(test)]
#[path = "exposure_tests.rs"]
mod exposure_tests;

#[cfg(test)]
#[path = "snapshot_tests.rs"]
mod snapshot_tests;
