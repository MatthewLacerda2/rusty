//! src/dev/lua_surface/bake.rs — the bake verbs: `Probe.Bake` (#241),
//! `Reflection.Bake` (#245), the one-button `Lighting.Bake` (#246), and the lightmap
//! pair `Lighting.BakeLightmaps` / `ClearLightmaps` (#438).
//!
//! Each is an authoring action, so each is added here, onto the namespace table `api`
//! registered, rather than in `api` itself (#737). The three GPU bakes return `true`
//! when a bake ran and `false` when no adapter was available (skipped gracefully); the
//! lightmap bake runs on the CPU and returns how many lightmaps it wrote. Each is
//! refused while the editor's Generate Lighting runs (#1602): the two would bake the
//! same probes and write the same files.

use std::cell::RefCell;

use mlua::Lua;

use super::super::{
    bake::generate_lighting::refuse_while_baking, bake::lighting as lighting_bake,
    bake::lightmap as lightmap_bake, bake::probe as probe_bake,
    bake::reflection as reflection_bake,
};
use super::generate::Overrides;
use crate::api::{global_table, put, ApiScopedCtx, Reg};
use crate::scene::lighting::placement::MIN_REGION;
use crate::scene::Scene;

/// Add `Bake` to the `Probe`, `Reflection` and `Lighting` tables.
pub(super) fn register<'scope>(
    lua: &Lua,
    scope: &'scope mlua::Scope<'scope, '_>,
    ctx: &ApiScopedCtx<'scope>,
) -> Reg {
    register_probe(lua, scope, ctx.scene)?;
    register_reflection(lua, scope, ctx.scene, ctx.scene_path)?;
    register_lighting(lua, scope, ctx)?;
    register_lightmaps(lua, scope, ctx)
}

/// `Probe.Bake()`: capture each probe's static surroundings to a cubemap and project
/// the bounce into SH (#241).
fn register_probe<'scope>(
    lua: &Lua,
    scope: &'scope mlua::Scope<'scope, '_>,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    put(
        &global_table(lua, "Probe")?,
        "Bake",
        scope.create_function(|_, ()| {
            let mut scene = scene.borrow_mut();
            refuse_while_baking(&scene).map_err(mlua::Error::external)?;
            probe_bake::bake_scene_probes(&mut scene).map_err(mlua::Error::external)
        }),
    )
}

/// `Reflection.Bake()` (#245): for each probe capture the static surroundings to a
/// cubemap, GGX-prefilter it into a roughness mip chain, write the KTX2 next to the
/// scene and point the probe at it. Errors when the scene is unsaved (nowhere to write).
fn register_reflection<'scope>(
    lua: &Lua,
    scope: &'scope mlua::Scope<'scope, '_>,
    scene: &'scope RefCell<Scene>,
    scene_path: &'scope RefCell<Option<String>>,
) -> Reg {
    put(
        &global_table(lua, "Reflection")?,
        "Bake",
        scope.create_function(|_, ()| {
            let mut scene = scene.borrow_mut();
            let path = scene_path.borrow();
            refuse_while_baking(&scene).map_err(mlua::Error::external)?;
            reflection_bake::bake_scene_reflections(&mut scene, path.as_deref())
                .map_err(mlua::Error::external)
        }),
    )
}

/// `Lighting.Bake([probeSpacing], [reflectionRegion])`: auto-place then bake
/// both sets (#246), the probe step of the editor's Generate Lighting button (#832).
/// `true` when at least one bake ran on the GPU; errors only when a bake's own
/// contract fails (reflections need a saved scene path).
fn register_lighting<'scope>(
    lua: &Lua,
    scope: &'scope mlua::Scope<'scope, '_>,
    ctx: &ApiScopedCtx<'scope>,
) -> Reg {
    type BakeArgs = (Option<f32>, Option<f32>);
    let (scene, scene_path, nav, console) = (ctx.scene, ctx.scene_path, ctx.nav, ctx.console);
    put(
        &global_table(lua, "Lighting")?,
        "Bake",
        scope.create_function(move |_, (spacing, region): BakeArgs| {
            let mut scene = scene.borrow_mut();
            let path = scene_path.borrow();
            let nav = nav.borrow();
            let params = bake_params(spacing, region)?;
            let mut say = |line| console.borrow_mut().info(line);
            let report = lighting_bake::bake_lighting(
                &mut scene,
                path.as_deref(),
                Some(&nav),
                params,
                &mut say,
            )
            .map_err(mlua::Error::external)?;
            Ok(report.ran_light_bake || report.ran_reflection_bake)
        }),
    )
}

/// Build bake params from the optional Lua arguments, falling back to the defaults.
/// A `reflectionRegion` under [`MIN_REGION`] (or not a number) is refused: the
/// probe count grows with its inverse square.
fn bake_params(
    spacing: Option<f32>,
    region: Option<f32>,
) -> mlua::Result<lighting_bake::LightingBakeParams> {
    let mut params = lighting_bake::LightingBakeParams::default();
    params.probe_spacing = spacing.unwrap_or(params.probe_spacing);
    let region = region.unwrap_or(params.reflection.region);
    if region.is_nan() || region < MIN_REGION {
        return Err(mlua::Error::runtime(format!(
            "Lighting.Bake: reflectionRegion {region} is under the {MIN_REGION} m minimum"
        )));
    }
    params.reflection.region = region;
    Ok(params)
}

/// `Lighting.BakeLightmaps([texelsPerUnit], [samples], [bounces], [seed],
/// [directional])` bakes every static mesh with a lightmap UV and returns how many
/// lightmaps it wrote (#438); `directional` (#810) adds the direction pages that let
/// normal maps reshape baked light. An argument left out takes the scene's lighting
/// settings (#832).
/// `Lighting.ClearLightmaps()` drops them all, back to probe / ambient lighting. The
/// lightmap step of the editor's Generate Lighting button (#832).
fn register_lightmaps<'scope>(
    lua: &Lua,
    scope: &'scope mlua::Scope<'scope, '_>,
    ctx: &ApiScopedCtx<'scope>,
) -> Reg {
    type Args = (
        Option<f32>,
        Option<u32>,
        Option<u32>,
        Option<u64>,
        Option<bool>,
    );
    let (scene, scene_path) = (ctx.scene, ctx.scene_path);
    let table = global_table(lua, "Lighting")?;
    put(
        &table,
        "BakeLightmaps",
        scope.create_function(
            move |_, (texels_per_unit, samples, bounces, seed, directional): Args| {
                let overrides = Overrides {
                    texels_per_unit,
                    samples,
                    bounces,
                    seed,
                    directional,
                };
                let settings = overrides.apply(scene.borrow().lighting_settings.lightmaps);
                let path = scene_path.borrow();
                let mut scene = scene.borrow_mut();
                refuse_while_baking(&scene).map_err(mlua::Error::external)?;
                lightmap_bake::bake_scene_lightmaps(&mut scene, path.as_deref(), &settings)
                    .map_err(mlua::Error::external)
            },
        ),
    )?;
    put(
        &table,
        "ClearLightmaps",
        scope.create_function(move |_, ()| {
            scene.borrow_mut().lightmaps.clear();
            Ok(())
        }),
    )
}
