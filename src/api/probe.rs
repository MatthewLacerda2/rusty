//! src/api/probe.rs — `Probe` namespace.
//!
//! Placement + query over the scene-level light-probe dataset (`Scene::probes`).
//! Probes are NOT a first-class component (they're a scene dataset, like the
//! material library), so this namespace operates on the volume as a whole rather
//! than on an entity id: add/move/remove single probes, fill a regular grid, run the
//! analytic fill, and query the interpolated SH irradiance at a world position.
//!
//! Editor↔API parity: every probe-placement action a user can take in the editor
//! has its mirror here. The heavy bake (#241) will reuse `Fill`/the analytic path.

use std::cell::RefCell;

use glam::Vec3;
use mlua::Lua;

use super::{put, Reg};
use crate::scene::Scene;

/// Register the `Probe` namespace onto `lua`.
pub fn register<'lua, 'scope>(
    lua: &'lua Lua,
    scope: &mlua::Scope<'lua, 'scope>,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;

    register_placement(scope, &table, scene)?;
    register_fill(scope, &table, scene)?;
    register_query(scope, &table, scene)?;

    lua.globals().set("Probe", table).map_err(|e| e.to_string())
}

/// `Add` / `Move` / `Remove` / `Clear` / `Count` over single probes.
fn register_placement<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    put(
        table,
        "Add",
        scope.create_function(|_, (x, y, z): (f32, f32, f32)| {
            let mut scene = scene.borrow_mut();
            Ok(scene.probes.add_probe(Vec3::new(x, y, z)) as u32)
        }),
    )?;

    put(
        table,
        "Move",
        scope.create_function(|_, (index, x, y, z): (u32, f32, f32, f32)| {
            let mut scene = scene.borrow_mut();
            scene.probes.move_probe(index as usize, Vec3::new(x, y, z));
            Ok(())
        }),
    )?;

    put(
        table,
        "Remove",
        scope.create_function(|_, index: u32| {
            let mut scene = scene.borrow_mut();
            scene.probes.remove_probe(index as usize);
            Ok(())
        }),
    )?;

    put(
        table,
        "Clear",
        scope.create_function(|_, ()| {
            scene.borrow_mut().probes.clear();
            Ok(())
        }),
    )?;

    put(
        table,
        "Count",
        scope.create_function(|_, ()| Ok(scene.borrow().probes.probes.len() as u32)),
    )
}

/// `FillGrid` (bounds + spacing), `BakeAnalytic` (the deterministic stand-in fill),
/// and the dev-only `Bake` (the real rendered-capture → SH bake, #241).
fn register_fill<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    type GridArgs = (f32, f32, f32, f32, f32, f32, f32);
    put(
        table,
        "FillGrid",
        scope.create_function(
            |_, (min_x, min_y, min_z, max_x, max_y, max_z, spacing): GridArgs| {
                let mut scene = scene.borrow_mut();
                scene.probes.fill_grid(
                    Vec3::new(min_x, min_y, min_z),
                    Vec3::new(max_x, max_y, max_z),
                    spacing,
                );
                Ok(())
            },
        ),
    )?;

    put(
        table,
        "BakeAnalytic",
        scope.create_function(|_, ()| {
            let mut scene = scene.borrow_mut();
            crate::scene::lighting::probe_fill::analytic_fill(&mut scene);
            Ok(())
        }),
    )?;

    register_bake(scope, table, scene)
}

/// The real bake (`Probe.Bake()`): capture each probe's static surroundings to a
/// cubemap and project the bounce into SH (#241). Dev-only — it drives the GPU via a
/// headless renderer — returning `true` when the bake ran, `false` when no adapter was
/// available (skipped gracefully). With the `dev` feature off the entry is simply
/// absent, matching the stripped agentic layer.
#[cfg(feature = "dev")]
fn register_bake<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    put(
        table,
        "Bake",
        scope.create_function(|_, ()| {
            let mut scene = scene.borrow_mut();
            crate::dev::probe_bake::bake_scene_probes(&mut scene).map_err(mlua::Error::external)
        }),
    )
}

/// No-op `register_bake` when the `dev` feature is off — `Probe.Bake` is a dev action
/// and isn't registered in a ship build.
#[cfg(not(feature = "dev"))]
fn register_bake<'lua, 'scope>(
    _scope: &mlua::Scope<'lua, 'scope>,
    _table: &mlua::Table,
    _scene: &'scope RefCell<Scene>,
) -> Reg {
    Ok(())
}

/// `SampleIrradiance(x,y,z, nx,ny,nz)` — the interpolated probe irradiance for a
/// surface normal at a world position (what the shader reads), as linear RGB.
/// Returns black when there are no probes.
fn register_query<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    type SampleArgs = (f32, f32, f32, f32, f32, f32);
    put(
        table,
        "SampleIrradiance",
        scope.create_function(|_, (x, y, z, nx, ny, nz): SampleArgs| {
            let scene = scene.borrow();
            let irradiance = scene
                .probes
                .sample(Vec3::new(x, y, z))
                .map(|sh| sh.eval(Vec3::new(nx, ny, nz)))
                .unwrap_or(Vec3::ZERO);
            Ok((irradiance.x, irradiance.y, irradiance.z))
        }),
    )
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;

    fn empty_scene() -> RefCell<Scene> {
        RefCell::new(Scene::default())
    }

    #[test]
    fn add_count_remove() {
        let scene = empty_scene();
        let lua = Lua::new();
        lua.scope(|scope| {
            register(&lua, scope, &scene).unwrap();
            lua.load("Probe.Add(1, 2, 3)").exec().unwrap();
            lua.load("Probe.Add(4, 5, 6)").exec().unwrap();
            let n: u32 = lua.load("return Probe.Count()").eval().unwrap();
            assert_eq!(n, 2);
            lua.load("Probe.Remove(0)").exec().unwrap();
            let n: u32 = lua.load("return Probe.Count()").eval().unwrap();
            assert_eq!(n, 1);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn fill_bake_and_sample() {
        let scene = empty_scene();
        {
            let mut s = scene.borrow_mut();
            s.ambient_color = Vec3::new(0.3, 0.3, 0.3);
            s.ambient_intensity = 1.0;
        }
        let lua = Lua::new();
        lua.scope(|scope| {
            register(&lua, scope, &scene).unwrap();
            lua.load("Probe.FillGrid(0,0,0, 2,2,2, 1)").exec().unwrap();
            lua.load("Probe.BakeAnalytic()").exec().unwrap();
            let (r, _g, _b): (f32, f32, f32) = lua
                .load("return Probe.SampleIrradiance(1,1,1, 0,1,0)")
                .eval()
                .unwrap();
            assert!(r > 0.0, "expected non-zero baked irradiance, got {r}");
            Ok(())
        })
        .unwrap();
    }

    /// `Probe.Bake()` returns a boolean (`true` when the GPU bake ran, `false` when no
    /// adapter is available) and never errors — adapter-independent, so it passes on a
    /// GPU-less CI host as well as one with lavapipe. The deeper "produces a directional
    /// coloured bounce" assertion lives in `render::probe_bake_tests`, which owns the
    /// scene + geometry; here we only verify the namespace wiring and the skip contract.
    #[cfg(feature = "dev")]
    #[test]
    fn bake_returns_bool_and_skips_gracefully() {
        let scene = empty_scene();
        let lua = Lua::new();
        lua.scope(|scope| {
            register(&lua, scope, &scene).unwrap();
            lua.load("Probe.FillGrid(0,0,0, 1,1,1, 1)").exec().unwrap();
            let ran: bool = lua.load("return Probe.Bake()").eval().unwrap();
            // A `bool` came back without an error — that's the wiring + skip contract.
            let _ = ran;
            Ok(())
        })
        .unwrap();
    }
}
