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

use glam::Vec3;
use mlua::Lua;

use super::{put, Reg};
use crate::api::ApiCell;
use crate::scene::Scene;

/// Register the `Probe` namespace onto `lua`.
pub fn register(lua: &Lua, scene: ApiCell<Scene>) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;

    register_placement(lua, &table, scene)?;
    register_manual(lua, &table, scene)?;
    register_fill(lua, &table, scene)?;
    register_query(lua, &table, scene)?;

    lua.globals().set("Probe", table).map_err(|e| e.to_string())
}

/// `Add` / `Move` / `Remove` / `Clear` / `Count` over single probes.
fn register_placement(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "Add",
        lua.create_function(move |lua, (x, y, z): (f32, f32, f32)| {
            let scene = &*scene.get(lua)?;
            let mut scene = scene.borrow_mut();
            let i = scene.probes.add_probe(Vec3::new(x, y, z));
            // Placed by hand: the bake's auto-placement keeps it (#1737).
            scene.probes.set_manual(i, true);
            Ok(i as u32)
        }),
    )?;

    put(
        table,
        "Move",
        lua.create_function(move |lua, (index, x, y, z): (u32, f32, f32, f32)| {
            let scene = &*scene.get(lua)?;
            let mut scene = scene.borrow_mut();
            scene.probes.move_probe(index as usize, Vec3::new(x, y, z));
            // A hand edit: the next bake keeps the probe where it was put (#1737).
            scene.probes.set_manual(index as usize, true);
            Ok(())
        }),
    )?;

    put(
        table,
        "Remove",
        lua.create_function(move |lua, index: u32| {
            let scene = &*scene.get(lua)?;
            let mut scene = scene.borrow_mut();
            scene.probes.remove_probe(index as usize);
            Ok(())
        }),
    )?;

    put(
        table,
        "Clear",
        lua.create_function(move |lua, ()| {
            scene.get(lua)?.borrow_mut().probes.clear();
            Ok(())
        }),
    )?;

    put(
        table,
        "Count",
        lua.create_function(move |lua, ()| {
            Ok(scene.get(lua)?.borrow().probes.probes().len() as u32)
        }),
    )
}

/// `SetManual` / `IsManual`: whether a bake's auto-placement keeps a probe (#1737).
fn register_manual(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "SetManual",
        lua.create_function(move |lua, (index, manual): (u32, bool)| {
            scene
                .get(lua)?
                .borrow_mut()
                .probes
                .set_manual(index as usize, manual);
            Ok(())
        }),
    )?;

    put(
        table,
        "IsManual",
        lua.create_function(move |lua, index: u32| {
            let scene = &*scene.get(lua)?;
            let scene = scene.borrow();
            Ok(scene
                .probes
                .probes()
                .get(index as usize)
                .is_some_and(|p| p.manual))
        }),
    )
}

/// `FillGrid` (bounds + spacing), `BakeAnalytic` (the deterministic stand-in fill),
/// (the dev-only `Bake`, the real rendered-capture → SH bake of #241, is installed onto
/// this table by `dev`, #737).
fn register_fill(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    type GridArgs = (f32, f32, f32, f32, f32, f32, f32);
    put(
        table,
        "FillGrid",
        lua.create_function(
            move |lua, (min_x, min_y, min_z, max_x, max_y, max_z, spacing): GridArgs| {
                let scene = &*scene.get(lua)?;
                let mut scene = scene.borrow_mut();
                scene.probes.fill_grid(
                    Vec3::new(min_x, min_y, min_z),
                    Vec3::new(max_x, max_y, max_z),
                    spacing,
                );
                // A grid asked for by hand is manual, so a bake keeps it (#1737).
                scene
                    .probes
                    .probes_mut()
                    .iter_mut()
                    .for_each(|p| p.manual = true);
                Ok(())
            },
        ),
    )?;

    put(
        table,
        "BakeAnalytic",
        lua.create_function(move |lua, ()| {
            let scene = &*scene.get(lua)?;
            let mut scene = scene.borrow_mut();
            crate::scene::lighting::probe_fill::analytic_fill(&mut scene);
            Ok(())
        }),
    )
}

/// `SampleIrradiance(x,y,z, nx,ny,nz)` — the interpolated probe irradiance for a
/// surface normal at a world position (what the shader reads), as linear RGB.
/// Returns black when there are no probes.
fn register_query(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    type SampleArgs = (f32, f32, f32, f32, f32, f32);
    put(
        table,
        "SampleIrradiance",
        lua.create_function(move |lua, (x, y, z, nx, ny, nz): SampleArgs| {
            let scene = &*scene.get(lua)?;
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
    use std::rc::Rc;

    use super::*;

    fn empty_scene() -> Rc<RefCell<Scene>> {
        Rc::new(RefCell::new(Scene::default()))
    }

    #[test]
    fn add_count_remove() {
        let scene = empty_scene();
        let lua = Lua::new();
        register(&lua, crate::api::bind(&lua, &scene)).unwrap();
        lua.load("Probe.Add(1, 2, 3)").exec().unwrap();
        lua.load("Probe.Add(4, 5, 6)").exec().unwrap();
        let n: u32 = lua.load("return Probe.Count()").eval().unwrap();
        assert_eq!(n, 2);
        lua.load("Probe.Remove(0)").exec().unwrap();
        let n: u32 = lua.load("return Probe.Count()").eval().unwrap();
        assert_eq!(n, 1);
    }

    #[test]
    fn added_and_filled_probes_are_manual() {
        let scene = empty_scene();
        let lua = Lua::new();
        let run = || -> mlua::Result<()> {
            register(&lua, crate::api::bind(&lua, &scene)).unwrap();
            let manual: bool = lua
                .load("Probe.Add(1, 2, 3) return Probe.IsManual(0)")
                .eval()?;
            assert!(manual, "Add places by hand (#1737)");
            let after: bool = lua
                .load("Probe.SetManual(0, false) return Probe.IsManual(0)")
                .eval()?;
            assert!(!after);
            assert!(
                !lua.load("return Probe.IsManual(9)").eval::<bool>()?,
                "out of range"
            );
            let moved: bool = lua
                .load("Probe.Move(0, 4, 4, 4) return Probe.IsManual(0)")
                .eval()?;
            assert!(moved, "a hand move makes it manual");
            lua.load("Probe.FillGrid(0,0,0, 1,0,0, 1)").exec()?;
            Ok(())
        };
        run().unwrap();
        assert!(scene.borrow().probes.probes().iter().all(|p| p.manual));
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
        register(&lua, crate::api::bind(&lua, &scene)).unwrap();
        lua.load("Probe.FillGrid(0,0,0, 2,2,2, 1)").exec().unwrap();
        lua.load("Probe.BakeAnalytic()").exec().unwrap();
        let (r, _g, _b): (f32, f32, f32) = lua
            .load("return Probe.SampleIrradiance(1,1,1, 0,1,0)")
            .eval()
            .unwrap();
        assert!(r > 0.0, "expected non-zero baked irradiance, got {r}");
    }
}
