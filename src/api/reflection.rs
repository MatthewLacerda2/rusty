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

use glam::Vec3;
use mlua::{Lua, Table};

use super::{put, Reg};
use crate::api::ApiCell;
use crate::scene::{ReflectionProbe, Scene};

/// Register the `Reflection` namespace onto `lua`. `scene_path` is the saved scene file
/// (if any) — the dev-only `Reflection.Bake()` (installed by `dev`, #737) writes its
/// cubemaps into its own `<scene>.reflections/` folder.
pub fn register(lua: &Lua, scene: ApiCell<Scene>) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;

    register_placement(lua, &table, scene)?;
    register_config(lua, &table, scene)?;
    register_settings(lua, &table, scene)?;

    lua.globals()
        .set("Reflection", table)
        .map_err(|e| e.to_string())
}

/// `Add` / `Move` over single probes, then [`register_list`].
fn register_placement(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    type AddArgs = (f32, f32, f32, f32, f32, f32);
    put(
        table,
        "Add",
        lua.create_function(move |lua, (x, y, z, hx, hy, hz): AddArgs| {
            let scene = &*scene.get(lua)?;
            let mut scene = scene.borrow_mut();
            let set = &mut scene.reflection_probes;
            let i = set.add(Vec3::new(x, y, z), Vec3::new(hx, hy, hz));
            // Placed by hand: the bake's auto-placement keeps it (#1737).
            set.edit(i, |p| p.manual = true);
            Ok(i as u32)
        }),
    )?;

    put(
        table,
        "Move",
        lua.create_function(move |lua, (index, x, y, z): (u32, f32, f32, f32)| {
            let scene = &*scene.get(lua)?;
            let mut scene = scene.borrow_mut();
            let set = &mut scene.reflection_probes;
            set.move_probe(index as usize, Vec3::new(x, y, z));
            hand_edited(set, index);
            Ok(())
        }),
    )?;
    register_list(lua, table, scene)
}

/// `Remove` / `Clear` / `Count`: the probe list itself.
fn register_list(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "Remove",
        lua.create_function(move |lua, index: u32| {
            scene
                .get(lua)?
                .borrow_mut()
                .reflection_probes
                .remove(index as usize);
            Ok(())
        }),
    )?;

    put(
        table,
        "Clear",
        lua.create_function(move |lua, ()| {
            scene.get(lua)?.borrow_mut().reflection_probes.clear();
            Ok(())
        }),
    )?;

    put(
        table,
        "Count",
        lua.create_function(move |lua, ()| {
            Ok(scene.get(lua)?.borrow().reflection_probes.len() as u32)
        }),
    )
}

/// `SetBox` (explicit parallax corners), `SetFallback` / `GetFallback` (Unity's
/// default reflection probe, #1697) and `SetCubemap` (point at a baked KTX2).
fn register_config(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    type BoxArgs = (u32, f32, f32, f32, f32, f32, f32);
    put(
        table,
        "SetBox",
        lua.create_function(
            move |lua, (index, min_x, min_y, min_z, max_x, max_y, max_z): BoxArgs| {
                let scene = &*scene.get(lua)?;
                let mut scene = scene.borrow_mut();
                let set = &mut scene.reflection_probes;
                let (min, max) = (
                    Vec3::new(min_x, min_y, min_z),
                    Vec3::new(max_x, max_y, max_z),
                );
                set.set_box(index as usize, min, max);
                hand_edited(set, index);
                Ok(())
            },
        ),
    )?;

    put(
        table,
        "SetFallback",
        lua.create_function(move |lua, index: Option<u32>| {
            let scene = &*scene.get(lua)?;
            let set = &mut scene.borrow_mut().reflection_probes;
            Ok(set.set_fallback(index.map(|i| i as usize)))
        }),
    )?;
    put(
        table,
        "GetFallback",
        lua.create_function(move |lua, ()| {
            let fallback = scene.get(lua)?.borrow().reflection_probes.fallback();
            Ok(fallback.map(|i| i as u32))
        }),
    )?;

    put(
        table,
        "SetCubemap",
        lua.create_function(move |lua, (index, path): (u32, String)| {
            let scene = &*scene.get(lua)?;
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
fn register_settings(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "Set",
        lua.create_function(move |lua, (index, t): (u32, Table)| {
            let scene = &*scene.get(lua)?;
            let mut scene = scene.borrow_mut();
            let set = &mut scene.reflection_probes;
            let Some(mut probe) = set.probes.get(index as usize).cloned() else {
                return Ok(());
            };
            // Every field is read before any is written: a bad one changes nothing.
            // A hand edit makes the probe manual (#1737). `manual = false` un-marks
            // only a probe that was manual already: a Get-modify-Set round trip of a
            // placed probe carries the `false` it read, and must keep the edit.
            let was_manual = probe.manual;
            write_fields(&mut probe, &t)?;
            probe.manual |= !was_manual;
            set.edit(index as usize, |p| *p = probe);
            Ok(())
        }),
    )?;
    put(
        table,
        "Get",
        lua.create_function(move |lua, index: u32| {
            let scene = &*scene.get(lua)?;
            let scene = scene.borrow();
            match scene.reflection_probes.probes.get(index as usize) {
                Some(p) => read_fields(lua, p).map(Some),
                None => Ok(None),
            }
        }),
    )
}

/// Mark probe `index` manual after a hand edit: the next bake keeps it as it was
/// left (#1737). The bake's own writes never come through here.
fn hand_edited(set: &mut crate::scene::ReflectionProbeSet, index: u32) {
    set.edit(index as usize, |p| p.manual = true);
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
    t.set("manual", p.manual)?;
    t.set("fallback", p.fallback)?;
    t.set("normalised", p.irradiance.is_some())?;
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
        // A new cube drops the old one's stored irradiance (#1697).
        if v != p.cubemap_path {
            p.irradiance = None;
        }
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
    set(t, "manual", &mut p.manual)?;
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
#[path = "reflection_fallback_tests.rs"]
mod fallback_tests;
#[cfg(test)]
#[path = "reflection_tests.rs"]
mod tests;
