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
//! add, move, resize the parallax box, point at a cubemap, edit Unity's per-probe
//! settings (`Set`/`Get`, #1584), remove — has its mirror here. The cubemap bake itself is a separate later issue (#245); until then a probe
//! can be placed and pointed at a not-yet-baked path, and the runtime falls back to the
//! skybox.

use std::cell::RefCell;

use glam::Vec3;
use mlua::{Lua, Table};

use super::{put, Reg};
use crate::scene::{ReflectionProbe, Scene};

/// Register the `Reflection` namespace onto `lua`. `scene_path` is the saved scene file
/// (if any) — the dev-only `Reflection.Bake()` (installed by `dev`, #737) writes its
/// cubemaps next to it.
pub fn register<'scope>(
    lua: &Lua,
    scope: &'scope mlua::Scope<'scope, '_>,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;

    register_placement(scope, &table, scene)?;
    register_config(scope, &table, scene)?;
    register_settings(scope, &table, scene)?;

    lua.globals()
        .set("Reflection", table)
        .map_err(|e| e.to_string())
}

/// `Add` / `Move` / `Remove` / `Clear` / `Count` over single probes.
fn register_placement<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
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
fn register_config<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
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

/// `Set(index, {…})` / `Get(index)`: every field of one probe (#1584), Unity's
/// Reflection Probe settings among them.
fn register_settings<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    put(
        table,
        "Set",
        scope.create_function(|_, (index, t): (u32, Table)| {
            let mut scene = scene.borrow_mut();
            let set = &mut scene.reflection_probes;
            let Some(mut probe) = set.probes.get(index as usize).cloned() else {
                return Ok(());
            };
            // Every field is read before any is written: a bad one changes nothing.
            write_fields(&mut probe, &t)?;
            set.edit(index as usize, |p| *p = probe);
            Ok(())
        }),
    )?;
    put(
        table,
        "Get",
        scope.create_function(|lua, index: u32| {
            let scene = scene.borrow();
            match scene.reflection_probes.probes.get(index as usize) {
                Some(p) => read_fields(lua, p).map(Some),
                None => Ok(None),
            }
        }),
    )
}

/// The `{ x, y, z }` table of `v`.
fn vec_table(lua: &Lua, v: Vec3) -> mlua::Result<Table> {
    let t = lua.create_table()?;
    t.set("x", v.x)?;
    t.set("y", v.y)?;
    t.set("z", v.z)?;
    Ok(t)
}

/// The `{ x, y, z }` table at `key` of `t`, if it is there.
fn vec_field(t: &Table, key: &str) -> mlua::Result<Option<Vec3>> {
    let Some(v) = t.get::<Option<Table>>(key)? else {
        return Ok(None);
    };
    Ok(Some(Vec3::new(v.get("x")?, v.get("y")?, v.get("z")?)))
}

/// Every field of `p` as `Reflection.Get` returns it.
fn read_fields(lua: &Lua, p: &ReflectionProbe) -> mlua::Result<Table> {
    let t = lua.create_table()?;
    t.set("position", vec_table(lua, p.position)?)?;
    t.set("boxMin", vec_table(lua, p.box_min)?)?;
    t.set("boxMax", vec_table(lua, p.box_max)?)?;
    t.set("cubemap", p.cubemap_path.as_str())?;
    t.set("boxProjection", p.box_projection)?;
    t.set("blendDistance", p.blend_distance)?;
    t.set("importance", p.importance)?;
    t.set("resolution", p.resolution)?;
    t.set("nearClip", p.near_clip)?;
    t.set("farClip", p.far_clip)?;
    t.set("shadows", p.shadows)?;
    t.set("intensity", p.intensity)?;
    Ok(t)
}

/// Write the fields `t` names into `p`; a field it leaves out keeps its value, and
/// `resolution = 0` goes back to the scene's default.
fn write_fields(p: &mut ReflectionProbe, t: &Table) -> mlua::Result<()> {
    if let Some(v) = vec_field(t, "position")? {
        p.position = v;
    }
    if let Some(v) = vec_field(t, "boxMin")? {
        p.box_min = v;
    }
    if let Some(v) = vec_field(t, "boxMax")? {
        p.box_max = v;
    }
    if let Some(v) = t.get::<Option<String>>("cubemap")? {
        p.cubemap_path = v;
    }
    if let Some(v) = t.get::<Option<u32>>("resolution")? {
        p.resolution = (v > 0).then_some(v);
    }
    set(t, "boxProjection", &mut p.box_projection)?;
    set(t, "blendDistance", &mut p.blend_distance)?;
    set(t, "importance", &mut p.importance)?;
    set(t, "nearClip", &mut p.near_clip)?;
    set(t, "farClip", &mut p.far_clip)?;
    set(t, "shadows", &mut p.shadows)?;
    set(t, "intensity", &mut p.intensity)
}

/// Overwrite `slot` with `t[key]` when it is there.
fn set<T: mlua::FromLua>(t: &Table, key: &str, slot: &mut T) -> mlua::Result<()> {
    if let Some(v) = t.get::<Option<T>>(key)? {
        *slot = v;
    }
    Ok(())
}

#[cfg(test)]
#[path = "reflection_tests.rs"]
mod tests;
