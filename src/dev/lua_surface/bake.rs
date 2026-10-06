//! src/dev/lua_surface/bake.rs — the bake verbs: `Probe.Bake` (#241),
//! `Reflection.Bake` (#245), the one-button `Lighting.Bake` (#246), and the lightmap
//! pair `Lighting.BakeLightmaps` / `ClearLightmaps` (#438).
//!
//! Each is an authoring action, so each is added here, onto the namespace table `api`
//! registered, rather than in `api` itself (#737). The three GPU bakes return `true`
//! when a bake ran and `false` when no adapter was available (skipped gracefully); the
//! lightmap bake runs on the CPU and returns how many lightmaps it wrote.

use std::cell::RefCell;

use mlua::Lua;

use super::super::{lighting_bake, lightmap_bake, probe_bake, reflection_bake};
use crate::api::{global_table, put, ApiScopedCtx, Reg};
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
            reflection_bake::bake_scene_reflections(&mut scene, path.as_deref())
                .map_err(mlua::Error::external)
        }),
    )
}

/// `Lighting.Bake([probeSpacing], [reflectionRegion], [cap])`: auto-place then bake
/// both sets (#246), through the same orchestration as the editor's "Bake Lighting"
/// button. `true` when at least one bake ran on the GPU; errors only when a bake's own
/// contract fails (reflections need a saved scene path).
fn register_lighting<'scope>(
    lua: &Lua,
    scope: &'scope mlua::Scope<'scope, '_>,
    ctx: &ApiScopedCtx<'scope>,
) -> Reg {
    type BakeArgs = (Option<f32>, Option<f32>, Option<u32>);
    let (scene, scene_path, nav) = (ctx.scene, ctx.scene_path, ctx.nav);
    put(
        &global_table(lua, "Lighting")?,
        "Bake",
        scope.create_function(move |_, (spacing, region, cap): BakeArgs| {
            let mut scene = scene.borrow_mut();
            let path = scene_path.borrow();
            let nav = nav.borrow();
            let params = bake_params(spacing, region, cap);
            let report =
                lighting_bake::bake_lighting(&mut scene, path.as_deref(), Some(&nav), params)
                    .map_err(mlua::Error::external)?;
            Ok(report.ran_light_bake || report.ran_reflection_bake)
        }),
    )
}

/// Build bake params from the optional Lua arguments, falling back to the defaults.
fn bake_params(
    spacing: Option<f32>,
    region: Option<f32>,
    cap: Option<u32>,
) -> lighting_bake::LightingBakeParams {
    let d = lighting_bake::LightingBakeParams::default();
    lighting_bake::LightingBakeParams {
        probe_spacing: spacing.unwrap_or(d.probe_spacing),
        reflection_region: region.unwrap_or(d.reflection_region),
        reflection_cap: cap.unwrap_or(d.reflection_cap),
    }
}

/// `Lighting.BakeLightmaps([texelsPerUnit], [samples], [bounces], [seed],
/// [directional])` bakes every static mesh with a lightmap UV and returns how many
/// lightmaps it wrote (#438); `directional` (default `true`, #810) adds the direction
/// pages that let normal maps reshape baked light.
/// `Lighting.ClearLightmaps()` drops them all, back to probe / ambient lighting. The
/// same path as the editor's "Bake Lightmaps" button.
fn register_lightmaps<'scope>(
    lua: &Lua,
    scope: &'scope mlua::Scope<'scope, '_>,
    ctx: &ApiScopedCtx<'scope>,
) -> Reg {
    use crate::scene::lighting::lightmap::BakeSettings;
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
            move |_, (density, samples, bounces, seed, directional): Args| {
                let d = BakeSettings::default();
                let settings = BakeSettings {
                    texels_per_unit: density.unwrap_or(d.texels_per_unit).max(0.01),
                    samples: samples.unwrap_or(d.samples).max(1),
                    bounces: bounces.unwrap_or(d.bounces).max(1),
                    seed: seed.unwrap_or(d.seed),
                    directional: directional.unwrap_or(d.directional),
                    ..d
                };
                let path = scene_path.borrow();
                let mut scene = scene.borrow_mut();
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
