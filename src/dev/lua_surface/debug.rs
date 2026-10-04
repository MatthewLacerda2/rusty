//! src/dev/lua_surface/debug.rs — `Debug` namespace (DEV-ONLY).
//!
//! `Debug.Log/Warn/Error`, the structured scene-read `Debug.Snapshot` /
//! `Debug.SnapshotEntity` (#180), the headless `Debug.Preview` asset shot (#353) and its
//! named-material twin `Debug.PreviewMaterial` (#404), and
//! the frame stats `Debug.Stats` (#433) — the agent's observation channels:
//! structured, visual, and performance. Registered
//! only in dev builds — stripped from the shipped game, like Unity `Debug.*` under
//! `[Conditional]`: it lives in `dev`, which installs it onto the surface (#737).

use mlua::{Lua, Table};

use crate::api::{put, snapshot, ApiScopedCtx, Reg};
use crate::dev::preview::{capture_asset, capture_material, PreviewOptions};
use crate::preview::PreviewMesh;
use crate::ui::UiView;

/// Register the `Debug` namespace onto `lua` (dev builds only).
pub fn register<'lua, 'scope>(
    lua: &'lua Lua,
    scope: &mlua::Scope<'lua, 'scope>,
    ctx: &ApiScopedCtx<'scope>,
) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;

    register_logging(scope, &table, ctx.console)?;
    register_snapshot(scope, &table, ctx)?;
    register_preview(lua, &table)?;
    register_preview_material(scope, &table, ctx.scene)?;
    register_stats(scope, &table, ctx.stats)?;

    lua.globals().set("Debug", table).map_err(|e| e.to_string())
}

/// `Debug.Log/Warn/Error` — append to the shared console buffer.
fn register_logging<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &mlua::Table,
    console: &'scope std::cell::RefCell<crate::scripting::ConsoleLogs>,
) -> Reg {
    put(
        table,
        "Log",
        scope.create_function(|_, msg: String| {
            console.borrow_mut().info(msg);
            Ok(())
        }),
    )?;
    put(
        table,
        "Warn",
        scope.create_function(|_, msg: String| {
            console.borrow_mut().warn(msg);
            Ok(())
        }),
    )?;
    put(
        table,
        "Error",
        scope.create_function(|_, msg: String| {
            console.borrow_mut().error(msg);
            Ok(())
        }),
    )
}

/// `Debug.Stats()` — the frame stats (#433) as a table: `frames`, each metric's
/// `{last, min, avg, max, samples}`, and `systems` (per-system CPU ms). Filled by the
/// dev layer's schedule probe; empty (`frames = 0`) where none is installed.
fn register_stats<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &Table,
    stats: &'scope std::cell::RefCell<crate::core::frame_stats::FrameStats>,
) -> Reg {
    put(
        table,
        "Stats",
        scope.create_function(move |lua, ()| crate::dev::stats::to_lua(lua, &stats.borrow())),
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
fn register_preview_material<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &Table,
    scene: &'scope std::cell::RefCell<crate::scene::Scene>,
) -> Reg {
    put(
        table,
        "PreviewMaterial",
        scope.create_function(
            move |_, (name, out_png, opts): (String, String, Option<Table>)| {
                let options = preview_options(opts.as_ref())?;
                let written = capture_material(&scene.borrow().materials, &name, &out_png, options)
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
    let mesh = match opts.get::<_, Option<String>>("mesh")? {
        Some(name) => parse_mesh(&name)?,
        None => defaults.mesh,
    };
    let resolution = opts
        .get::<_, Option<u32>>("resolution")?
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
fn register_snapshot<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &mlua::Table,
    ctx: &ApiScopedCtx<'scope>,
) -> Reg {
    let scene = ctx.scene;
    let camera = ctx.camera;
    let time = ctx.time;
    let is_playing = ctx.is_playing;
    let (screen, video) = (ctx.screen, ctx.video);
    let pixels = move || screen.borrow().pixels(&video.borrow());
    put(
        table,
        "Snapshot",
        scope.create_function(move |_, opts: Option<Table>| {
            // `{ bones = true }` includes skeleton bones, left out by default (#453).
            let bones = opts.and_then(|o| o.get::<_, Option<bool>>("bones").ok().flatten());
            let value = snapshot::world_value_with(
                &scene.borrow(),
                &camera.borrow(),
                time.borrow().frame_count,
                *is_playing.borrow(),
                pixels(),
                bones.unwrap_or(false),
            );
            Ok(serde_json::to_string_pretty(&value).unwrap_or_default())
        }),
    )?;

    let scene = ctx.scene;
    put(
        table,
        "SnapshotEntity",
        scope.create_function(move |_, id: u32| {
            let scene = scene.borrow();
            let world_matrix = scene.compute_world_matrix(id);
            let value = if scene.world.contains(id) {
                let view = UiView::with_camera(pixels(), camera.borrow().clone());
                snapshot::entity_value(&scene, id, world_matrix, &view)
            } else {
                serde_json::Value::Null
            };
            Ok(serde_json::to_string_pretty(&value).unwrap_or_default())
        }),
    )
}
