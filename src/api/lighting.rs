//! src/api/lighting.rs — `Lighting` namespace: the one-button bake.
//!
//! `Lighting.Bake()` is the "place well + bake right now" workflow (#246): in one call
//! it auto-places light probes and reflection probes from the static scene (unless a set
//! is already manually authored), then runs the existing full-bounce light-probe bake
//! (#250) and GGX reflection-probe bake (#252). It is the orchestration over the `Probe`
//! and `Reflection` namespaces — manual placement on those stays available; this is the
//! batteries-included path.
//!
//! Editor↔API parity: the editor's "Bake Lighting" button routes through the SAME
//! orchestration (`dev::lighting_bake::bake_lighting`), so the button and this verb never
//! drift. The bake is dev-only (it drives a headless GPU and writes authoring artifacts),
//! so `Bake` is registered only under the `dev` feature — in a ship build the `Lighting`
//! table is present but empty, matching the stripped agentic layer.

use std::cell::RefCell;

use mlua::Lua;

use super::Reg;
#[cfg(feature = "dev")]
use crate::navigation::NavigationGraph;
use crate::scene::Scene;

/// Register the `Lighting` namespace onto `lua`. `scene_path` is the saved scene file
/// (reflections write their cubemaps beside it); `nav` is the baked nav surface placement
/// reads its walkable bounds from.
pub fn register<'lua, 'scope>(
    lua: &'lua Lua,
    scope: &mlua::Scope<'lua, 'scope>,
    scene: &'scope RefCell<Scene>,
    scene_path: &'scope RefCell<Option<String>>,
    nav: &'scope RefCell<crate::navigation::NavigationGraph>,
) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;
    register_bake(scope, &table, scene, scene_path, nav)?;
    lua.globals()
        .set("Lighting", table)
        .map_err(|e| e.to_string())
}

/// The one-button bake (`Lighting.Bake([probeSpacing], [reflectionRegion], [cap])`):
/// auto-place then bake both sets (#246). Returns `true` when at least one bake ran on
/// the GPU, `false` when both skipped for want of an adapter; errors only when a bake's
/// own contract fails (reflections need a saved scene path). Dev-only.
#[cfg(feature = "dev")]
fn register_bake<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
    scene_path: &'scope RefCell<Option<String>>,
    nav: &'scope RefCell<NavigationGraph>,
) -> Reg {
    type BakeArgs = (Option<f32>, Option<f32>, Option<u32>);
    super::put(
        table,
        "Bake",
        scope.create_function(move |_, (spacing, region, cap): BakeArgs| {
            let mut scene = scene.borrow_mut();
            let path = scene_path.borrow();
            let nav = nav.borrow();
            let params = bake_params(spacing, region, cap);
            let report = crate::dev::lighting_bake::bake_lighting(
                &mut scene,
                path.as_deref(),
                Some(&nav),
                params,
            )
            .map_err(mlua::Error::external)?;
            Ok(report.ran_light_bake || report.ran_reflection_bake)
        }),
    )
}

/// Build bake params from the optional Lua arguments, falling back to the defaults.
#[cfg(feature = "dev")]
fn bake_params(
    spacing: Option<f32>,
    region: Option<f32>,
    cap: Option<u32>,
) -> crate::dev::lighting_bake::LightingBakeParams {
    let d = crate::dev::lighting_bake::LightingBakeParams::default();
    crate::dev::lighting_bake::LightingBakeParams {
        probe_spacing: spacing.unwrap_or(d.probe_spacing),
        reflection_region: region.unwrap_or(d.reflection_region),
        reflection_cap: cap.unwrap_or(d.reflection_cap),
    }
}

/// No-op `register_bake` with the `dev` feature off — `Lighting.Bake` is a dev action,
/// so the namespace table is present but carries no verb in a ship build.
#[cfg(not(feature = "dev"))]
fn register_bake<'lua, 'scope>(
    _scope: &mlua::Scope<'lua, 'scope>,
    _table: &mlua::Table,
    _scene: &'scope RefCell<Scene>,
    _scene_path: &'scope RefCell<Option<String>>,
    _nav: &'scope RefCell<crate::navigation::NavigationGraph>,
) -> Reg {
    Ok(())
}

#[cfg(all(test, feature = "dev"))]
mod tests {
    use std::cell::RefCell;

    use super::*;

    #[test]
    fn bake_on_empty_scene_runs_and_returns_bool() {
        // No static geometry → nothing to place → no probes → both bakes are no-ops
        // that report `true` (successful), so the verb returns `false` (nothing ran on
        // a GPU) WITHOUT erroring — the wiring + skip contract.
        let scene = RefCell::new(Scene::default());
        let path: RefCell<Option<String>> = RefCell::new(None);
        let nav = RefCell::new(NavigationGraph::new(0.0, 0.0, 0.0, 0.0, 1.0));
        let lua = Lua::new();
        lua.scope(|scope| {
            register(&lua, scope, &scene, &path, &nav).unwrap();
            let ran: bool = lua.load("return Lighting.Bake()").eval().unwrap();
            let _ = ran;
            Ok(())
        })
        .unwrap();
    }
}
