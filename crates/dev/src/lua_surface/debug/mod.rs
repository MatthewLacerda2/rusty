//! `Debug`, dev-only like Unity's `Debug.*` under `[Conditional]`: a ship build has none.

mod audio;
mod draw;
mod frame_log;

use mlua::{Lua, Table};

use crate::preview::{capture_asset, capture_material, PreviewOptions};
use rusty_preview::PreviewMesh;
use rusty_scene::game_camera_from_scene;
use rusty_script::api::{put, snapshot, ApiCtx, Reg};
use rusty_script::scripting::Mark;

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
    frame_log::register(lua, &table, ctx)?;

    lua.globals().set("Debug", table).map_err(|e| e.to_string())
}

fn register_logging(
    lua: &mlua::Lua,
    table: &mlua::Table,
    console: rusty_script::api::ApiCell<rusty_script::scripting::ConsoleLogs>,
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
    put(
        table,
        "Mark",
        lua.create_function(move |lua, (label, value): (String, mlua::Value)| {
            let value = match value {
                mlua::Value::Nil => None,
                value => Some(crate::demo::plain_value(&value).map_err(|kind| {
                    let msg = format!("Debug.Mark: a value must be plain data, not a {kind}");
                    mlua::Error::RuntimeError(msg)
                })?),
            };
            console.get(lua)?.borrow_mut().mark(Mark { label, value });
            Ok(())
        }),
    )
}

fn register_stats(
    lua: &mlua::Lua,
    table: &Table,
    stats: rusty_script::api::ApiCell<rusty_core::frame_stats::FrameStats>,
) -> Reg {
    put(
        table,
        "Stats",
        lua.create_function(move |lua, opts: Option<Table>| {
            let window = crate::stats::window_opt(opts)?;
            stats.with(lua, |stats| match window {
                Some(n) => crate::stats::to_lua(lua, &stats.borrow().window(n)),
                None => crate::stats::to_lua(lua, &stats.borrow()),
            })
        }),
    )
}

/// Dev-only on purpose: a GPU readback is no sim input, so `Graphics.GetExposure` returns
/// the configured EV.
fn register_adapted_exposure(
    lua: &mlua::Lua,
    table: &Table,
    scene: rusty_script::api::ApiCell<rusty_scene::Scene>,
) -> Reg {
    put(
        table,
        "GetAdaptedExposure",
        lua.create_function(move |lua, ()| Ok(scene.get(lua)?.borrow().screen_exposure)),
    )
}

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

fn register_preview_material(
    lua: &mlua::Lua,
    table: &Table,
    scene: rusty_script::api::ApiCell<rusty_scene::Scene>,
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
    let repeats = match opts.get::<Option<i64>>("repeats")? {
        Some(n) if n >= 1 => Some(u32::try_from(n).unwrap_or(u32::MAX)),
        Some(n) => {
            return Err(mlua::Error::RuntimeError(format!(
                "preview repeats must be 1 or more, got {n}"
            )))
        }
        None => defaults.repeats,
    };
    Ok(PreviewOptions {
        mesh,
        resolution,
        repeats,
    })
}

fn parse_mesh(name: &str) -> mlua::Result<PreviewMesh> {
    PreviewMesh::parse(name).ok_or_else(|| {
        let expected = PreviewMesh::ALL.map(|m| format!("{:?}", m.name().to_lowercase()));
        mlua::Error::RuntimeError(format!(
            "unknown preview mesh {:?} — expected one of {}",
            name.to_lowercase(),
            expected.join(", ")
        ))
    })
}

fn register_snapshot(lua: &mlua::Lua, table: &mlua::Table, ctx: &ApiCtx) -> Reg {
    let scene = ctx.scene;
    let camera = ctx.camera;
    let time = ctx.time;
    let (screen, video) = (ctx.screen, ctx.video);
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
        lua.create_function(move |lua, id: rusty_script::api::entity_arg::EntityArg| {
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
