//! src/api/reflection.rs — `Reflection` namespace.
//!
//! Placement + query over the scene-level reflection-probe dataset
//! (`Scene::reflection_probes`). Reflection probes are the SPECULAR sibling of the
//! light probes the `Probe` namespace drives (#240): each one is a capture position, a
//! box volume for parallax correction, and a path to a baked cubemap. They are NOT a
//! first-class component (they're a scene dataset, like the material library), so this
//! namespace operates on the set as a whole, addressing probes by INDEX (0-based, in
//! placement order).
//!
//! Editor↔API parity: every reflection-probe action a user can take in the editor —
//! add, move, resize the parallax box, point at a cubemap, remove — has its mirror
//! here. The cubemap bake itself is a separate later issue (#245); until then a probe
//! can be placed and pointed at a not-yet-baked path, and the runtime falls back to the
//! skybox.

use std::cell::RefCell;

use glam::Vec3;
use mlua::Lua;

use super::{put, Reg};
use crate::scene::Scene;

/// Register the `Reflection` namespace onto `lua`. `scene_path` is the saved scene file
/// (if any) — the dev-only `Reflection.Bake()` writes its cubemaps next to it.
pub fn register<'lua, 'scope>(
    lua: &'lua Lua,
    scope: &mlua::Scope<'lua, 'scope>,
    scene: &'scope RefCell<Scene>,
    scene_path: &'scope RefCell<Option<String>>,
) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;

    register_placement(scope, &table, scene)?;
    register_config(scope, &table, scene)?;
    register_bake(scope, &table, scene, scene_path)?;

    lua.globals()
        .set("Reflection", table)
        .map_err(|e| e.to_string())
}

/// The reflection-probe bake (`Reflection.Bake()`, #245): for each probe capture the
/// static surroundings to a cubemap, GGX-prefilter it into a roughness mip chain, write
/// the KTX2 next to the scene and point the probe at it. Dev-only — it drives the GPU via
/// a headless renderer — returning `true` when the bake ran, `false` when no adapter was
/// available (skipped gracefully). Errors when the scene is unsaved (nowhere to write).
#[cfg(feature = "dev")]
fn register_bake<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
    scene_path: &'scope RefCell<Option<String>>,
) -> Reg {
    put(
        table,
        "Bake",
        scope.create_function(|_, ()| {
            let mut scene = scene.borrow_mut();
            let path = scene_path.borrow();
            crate::dev::reflection_bake::bake_scene_reflections(&mut scene, path.as_deref())
                .map_err(mlua::Error::external)
        }),
    )
}

/// No-op `register_bake` with the `dev` feature off — `Reflection.Bake` is a dev action
/// and isn't registered in a ship build.
#[cfg(not(feature = "dev"))]
fn register_bake<'lua, 'scope>(
    _scope: &mlua::Scope<'lua, 'scope>,
    _table: &mlua::Table,
    _scene: &'scope RefCell<Scene>,
    _scene_path: &'scope RefCell<Option<String>>,
) -> Reg {
    Ok(())
}

/// `Add` / `Move` / `Remove` / `Clear` / `Count` over single probes.
fn register_placement<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    type AddArgs = (f32, f32, f32, f32, f32, f32);
    put(
        table,
        "Add",
        scope.create_function(|_, (x, y, z, hx, hy, hz): AddArgs| {
            let mut scene = scene.borrow_mut();
            let i = scene
                .reflection_probes
                .add(Vec3::new(x, y, z), Vec3::new(hx, hy, hz));
            Ok(i as u32)
        }),
    )?;

    put(
        table,
        "Move",
        scope.create_function(|_, (index, x, y, z): (u32, f32, f32, f32)| {
            scene
                .borrow_mut()
                .reflection_probes
                .move_probe(index as usize, Vec3::new(x, y, z));
            Ok(())
        }),
    )?;

    put(
        table,
        "Remove",
        scope.create_function(|_, index: u32| {
            scene.borrow_mut().reflection_probes.remove(index as usize);
            Ok(())
        }),
    )?;

    put(
        table,
        "Clear",
        scope.create_function(|_, ()| {
            scene.borrow_mut().reflection_probes.clear();
            Ok(())
        }),
    )?;

    put(
        table,
        "Count",
        scope.create_function(|_, ()| Ok(scene.borrow().reflection_probes.len() as u32)),
    )
}

/// `SetBox` (explicit parallax corners) and `SetCubemap` (point at a baked KTX2).
fn register_config<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    type BoxArgs = (u32, f32, f32, f32, f32, f32, f32);
    put(
        table,
        "SetBox",
        scope.create_function(
            |_, (index, min_x, min_y, min_z, max_x, max_y, max_z): BoxArgs| {
                scene.borrow_mut().reflection_probes.set_box(
                    index as usize,
                    Vec3::new(min_x, min_y, min_z),
                    Vec3::new(max_x, max_y, max_z),
                );
                Ok(())
            },
        ),
    )?;

    put(
        table,
        "SetCubemap",
        scope.create_function(|_, (index, path): (u32, String)| {
            scene
                .borrow_mut()
                .reflection_probes
                .set_cubemap(index as usize, path);
            Ok(())
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

    fn no_path() -> RefCell<Option<String>> {
        RefCell::new(None)
    }

    #[test]
    fn add_count_remove() {
        let scene = empty_scene();
        let path = no_path();
        let lua = Lua::new();
        lua.scope(|scope| {
            register(&lua, scope, &scene, &path).unwrap();
            lua.load("Reflection.Add(0,0,0, 5,5,5)").exec().unwrap();
            lua.load("Reflection.Add(10,0,0, 5,5,5)").exec().unwrap();
            let n: u32 = lua.load("return Reflection.Count()").eval().unwrap();
            assert_eq!(n, 2);
            lua.load("Reflection.Remove(0)").exec().unwrap();
            let n: u32 = lua.load("return Reflection.Count()").eval().unwrap();
            assert_eq!(n, 1);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn set_box_and_cubemap() {
        let scene = empty_scene();
        let path = no_path();
        let lua = Lua::new();
        lua.scope(|scope| {
            register(&lua, scope, &scene, &path).unwrap();
            lua.load("Reflection.Add(0,0,0, 1,1,1)").exec().unwrap();
            lua.load("Reflection.SetBox(0, -3,-3,-3, 3,3,3)")
                .exec()
                .unwrap();
            lua.load("Reflection.SetCubemap(0, 'room.ktx2')")
                .exec()
                .unwrap();
            Ok(())
        })
        .unwrap();
        let s = scene.borrow();
        let p = &s.reflection_probes.probes[0];
        assert_eq!(p.box_min, Vec3::splat(-3.0));
        assert_eq!(p.box_max, Vec3::splat(3.0));
        assert_eq!(p.cubemap_path, "room.ktx2");
    }

    /// `Reflection.Bake()` is registered (dev-only) and a no-op with no probes returns
    /// `true`. The real GPU bake + file round-trip is covered in
    /// `render::reflection_bake_tests`, which owns the renderer; here we only assert the
    /// binding exists and threads through to the dev bridge.
    #[cfg(feature = "dev")]
    #[test]
    fn bake_no_probes_is_ok() {
        let scene = empty_scene();
        let path = RefCell::new(Some("scene.scene".to_string()));
        let lua = Lua::new();
        lua.scope(|scope| {
            register(&lua, scope, &scene, &path).unwrap();
            let ran: bool = lua.load("return Reflection.Bake()").eval().unwrap();
            assert!(ran, "no-probe bake should succeed");
            Ok(())
        })
        .unwrap();
    }
}
