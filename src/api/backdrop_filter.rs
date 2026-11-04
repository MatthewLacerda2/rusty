//! src/api/backdrop_filter.rs — `BackdropFilter` namespace (#426).
//!
//! Get/Set over an entity's `BackdropFilterComponent`: the frosted-glass blur
//! radius, tint, saturation and brightness. Whether one exists is
//! `Scene.AddComponent(id, "BackdropFilter")` / `RemoveComponent`. Setters route
//! through the shared `scene::authoring::ui::backdrop` ops the inspector card uses;
//! getters return the defaults without one, and setters are then no-ops.

use glam::Vec4;
use mlua::Lua;

use super::{put, Reg};
use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use crate::components::BackdropFilterComponent;
use crate::scene::authoring::ui::backdrop as ops;
use crate::scene::Scene;

type Get = fn(&BackdropFilterComponent) -> f32;
type Set = fn(&mut BackdropFilterComponent, f32);

/// `(suffix, getter, setter)` for the scalar `Get<X>` / `Set<X>` pairs.
const SCALARS: [(&str, Get, Set); 3] = [
    ("BlurRadius", |b| b.blur_radius, ops::set_blur_radius),
    ("Saturation", |b| b.saturation, ops::set_saturation),
    ("Brightness", |b| b.brightness, ops::set_brightness),
];

/// Register the `BackdropFilter` namespace onto `lua`.
pub fn register(lua: &Lua, scene: ApiCell<Scene>) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;
    register_scalars(lua, &table, scene)?;
    register_tint(lua, &table, scene)?;
    lua.globals()
        .set("BackdropFilter", table)
        .map_err(|e| e.to_string())
}

/// The [`SCALARS`] getter/setter pairs.
fn register_scalars(lua: &Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    for (suffix, get, set) in SCALARS {
        put(
            table,
            &format!("Get{suffix}"),
            lua.create_function(move |lua, id: EntityArg| {
                let scene = &*scene.get(lua)?;
                let scene = scene.borrow();
                let b = scene.world.backdrop_filter(id.read()).map(|b| (*b).clone());
                Ok(get(&b.unwrap_or_default()))
            }),
        )?;
        let verb = format!("Set{suffix}");
        put(
            table,
            &format!("Set{suffix}"),
            lua.create_function(move |lua, (id, v): (EntityArg, f32)| {
                let id = id.live("BackdropFilter", &verb)?;
                if let Some(mut b) = scene.get(lua)?.borrow_mut().world.backdrop_filter_mut(id) {
                    set(&mut b, v);
                }
                Ok(())
            }),
        )?;
    }
    Ok(())
}

/// `GetTint` / `SetTint`.
fn register_tint(lua: &Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "GetTint",
        lua.create_function(move |lua, id: EntityArg| {
            let scene = &*scene.get(lua)?;
            let scene = scene.borrow();
            let t = scene
                .world
                .backdrop_filter(id.read())
                .map_or(Vec4::ZERO, |b| b.tint);
            Ok((t.x, t.y, t.z, t.w))
        }),
    )?;
    put(
        table,
        "SetTint",
        lua.create_function(
            move |lua, (id, r, g, b, a): (EntityArg, f32, f32, f32, f32)| {
                let id = id.live("BackdropFilter", "SetTint")?;
                if let Some(mut f) = scene.get(lua)?.borrow_mut().world.backdrop_filter_mut(id) {
                    ops::set_tint(&mut f, Vec4::new(r, g, b, a));
                }
                Ok(())
            },
        ),
    )
}
