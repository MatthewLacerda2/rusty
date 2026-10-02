//! src/api/nav_modifier.rs — `NavMeshModifierVolume` namespace (#460).
//!
//! Unity's `NavMeshModifierVolume`: a box (centre and size, local to the entity)
//! that assigns a navigation area to the walkable surface inside it at bake time;
//! area `1` (`NotWalkable`) removes the surface. Look an area's id up with
//! `Navigation.GetAreaFromName`. Every setter routes through the shared
//! `scene::authoring::nav_modifier` ops the inspector card uses; the volume is a
//! bake input, so the next navmesh sync rebakes the cells it covers. Getters return
//! a neutral default without a volume; setters are then no-ops. Attach one with
//! `Scene.AddComponent(id, "NavMeshModifierVolume")`.

use std::cell::RefCell;

use glam::Vec3;
use mlua::{Lua, Table};

use super::{put, Reg};
use crate::components::NavMeshModifierVolumeComponent as Volume;
use crate::scene::authoring::nav_modifier as ops;
use crate::scene::Scene;

type SceneCell<'s> = &'s RefCell<Scene>;

/// Register the `NavMeshModifierVolume` namespace onto `lua`.
pub fn register<'lua, 'scope>(
    lua: &'lua Lua,
    scope: &mlua::Scope<'lua, 'scope>,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    let t = lua.create_table().map_err(|e| e.to_string())?;
    register_box(scope, &t, scene)?;
    register_options(scope, &t, scene)?;
    lua.globals()
        .set("NavMeshModifierVolume", t)
        .map_err(|e| e.to_string())
}

/// `id`'s volume, cloned, or `None` without one.
fn get(scene: SceneCell, id: u32) -> Option<Volume> {
    scene.borrow().world.nav_modifier(id).map(|v| v.clone())
}

/// Apply a shared op to `id`'s volume, if it has one.
fn write(scene: SceneCell, id: u32, op: impl FnOnce(&mut Volume)) {
    if let Some(mut v) = scene.borrow_mut().world.nav_modifier_mut(id) {
        op(&mut v);
    }
}

/// `GetCenter` / `SetCenter` and `GetSize` / `SetSize`, as `x, y, z`.
fn register_box<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    t: &Table<'lua>,
    scene: SceneCell<'scope>,
) -> Reg {
    type Field = (fn(&Volume) -> Vec3, fn(&mut Volume, Vec3));
    let fields: [(&str, Field); 2] = [
        ("Center", (|v| v.center, ops::set_center)),
        ("Size", (|v| v.size, ops::set_size)),
    ];
    for (name, (read, set)) in fields {
        let f = scope.create_function(move |_, id: u32| {
            let v = get(scene, id).map_or(Vec3::ZERO, |v| read(&v));
            Ok((v.x, v.y, v.z))
        });
        put(t, &format!("Get{name}"), f)?;
        let f = scope.create_function(move |_, (id, x, y, z): (u32, f32, f32, f32)| {
            write(scene, id, |v| set(v, Vec3::new(x, y, z)));
            Ok(())
        });
        put(t, &format!("Set{name}"), f)?;
    }
    Ok(())
}

/// `Active` and `Area` (an area id; `0` without a volume).
fn register_options<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    t: &Table<'lua>,
    scene: SceneCell<'scope>,
) -> Reg {
    let f = scope.create_function(move |_, id: u32| Ok(get(scene, id).is_some_and(|v| v.active)));
    put(t, "GetActive", f)?;
    let f = scope.create_function(move |_, (id, on): (u32, bool)| {
        write(scene, id, |v| ops::set_active(v, on));
        Ok(())
    });
    put(t, "SetActive", f)?;
    let f = scope.create_function(move |_, id: u32| Ok(get(scene, id).map_or(0, |v| v.area)));
    put(t, "GetArea", f)?;
    let f = scope.create_function(move |_, (id, area): (u32, i64)| {
        write(scene, id, |v| ops::set_area(v, area));
        Ok(())
    });
    put(t, "SetArea", f)
}

#[cfg(test)]
#[path = "nav_modifier_tests.rs"]
mod tests;
