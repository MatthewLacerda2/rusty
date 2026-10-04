//! src/dev/lua_surface/bake.rs — the GPU bake verbs: `Probe.Bake` (#241),
//! `Reflection.Bake` (#245) and the one-button `Lighting.Bake` (#246).
//!
//! Each drives a headless renderer, so each is added here, onto the namespace table
//! `api` registered, rather than in `api` itself (#737). All three return `true` when
//! a GPU bake ran and `false` when no adapter was available (skipped gracefully).

use std::cell::RefCell;

use mlua::Lua;

use super::super::{lighting_bake, probe_bake, reflection_bake};
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
    register_lighting(lua, scope, ctx)
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
