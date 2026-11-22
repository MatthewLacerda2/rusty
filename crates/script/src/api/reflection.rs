use glam::Vec3;
use mlua::{Lua, Table};

use super::{put, Reg};
use crate::api::ApiCell;
use rusty_components::transform::{euler_degrees, from_euler_degrees};
use rusty_scene::{ProbeShape, ReflectionProbe, Scene};

pub fn register(lua: &Lua, scene: ApiCell<Scene>) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;

    register_placement(lua, &table, scene)?;
    register_config(lua, &table, scene)?;
    register_settings(lua, &table, scene)?;

    lua.globals()
        .set("Reflection", table)
        .map_err(|e| e.to_string())
}

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

fn register_list(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "Remove",
        lua.create_function(move |lua, index: u32| {
            let scene = scene.get(lua)?;
            scene.borrow_mut().remove_reflection_probe(index as usize);
            Ok(())
        }),
    )?;

    put(
        table,
        "Clear",
        lua.create_function(move |lua, ()| {
            scene.get(lua)?.borrow_mut().clear_reflection_probes();
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

fn hand_edited(set: &mut rusty_scene::ReflectionProbeSet, index: u32) {
    set.edit(index as usize, |p| p.manual = true);
}

fn vec_table(lua: &Lua, v: Vec3) -> mlua::Result<Table> {
    let t = lua.create_table()?;
    t.set("x", v.x)?;
    t.set("y", v.y)?;
    t.set("z", v.z)?;
    Ok(t)
}

fn vec_field(t: &Table, key: &str) -> mlua::Result<Option<Vec3>> {
    let Some(v) = t.get::<Option<Table>>(key)? else {
        return Ok(None);
    };
    Ok(Some(Vec3::new(v.get("x")?, v.get("y")?, v.get("z")?)))
}

fn read_fields(lua: &Lua, p: &ReflectionProbe) -> mlua::Result<Table> {
    let t = lua.create_table()?;
    t.set("position", vec_table(lua, p.position)?)?;
    t.set("boxMin", vec_table(lua, p.box_min)?)?;
    t.set("boxMax", vec_table(lua, p.box_max)?)?;
    t.set("rotation", vec_table(lua, euler_degrees(p.rotation))?)?;
    t.set("shape", format!("{:?}", p.shape))?;
    t.set("radius", p.radius)?;
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
    if let Some(v) = vec_field(t, "rotation")?.filter(|&v| v != euler_degrees(p.rotation)) {
        p.rotation = from_euler_degrees(v);
    }
    if let Some(v) = t.get::<Option<String>>("shape")? {
        let bad = || mlua::Error::runtime(format!("shape {v:?}: \"Box\" or \"Sphere\""));
        p.shape = ProbeShape::named(&v).ok_or_else(bad)?;
    }
    set(t, "radius", &mut p.radius)?;
    if let Some(v) = t.get::<Option<String>>("cubemap")? {
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
