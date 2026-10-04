//! src/api/backdrop_filter.rs — `BackdropFilter` namespace (#426).
//!
//! Get/Set over an entity's `BackdropFilterComponent`: the frosted-glass blur
//! radius, tint, saturation and brightness. Whether one exists is
//! `Scene.AddComponent(id, "BackdropFilter")` / `RemoveComponent`. Setters route
//! through the shared `scene::authoring::backdrop` ops the inspector card uses;
//! getters return the defaults without one, and setters are then no-ops.

use std::cell::RefCell;

use glam::Vec4;
use mlua::Lua;

use super::{put, Reg};
use crate::components::BackdropFilterComponent;
use crate::scene::authoring::backdrop as ops;
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
pub fn register<'scope>(
    lua: &Lua,
    scope: &'scope mlua::Scope<'scope, '_>,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;
    for (suffix, get, set) in SCALARS {
        put(
            &table,
            &format!("Get{suffix}"),
            scope.create_function(move |_, id: u32| {
                let scene = scene.borrow();
                let b = scene.world.backdrop_filter(id).map(|b| (*b).clone());
                Ok(get(&b.unwrap_or_default()))
            }),
        )?;
        put(
            &table,
            &format!("Set{suffix}"),
            scope.create_function(move |_, (id, v): (u32, f32)| {
                if let Some(mut b) = scene.borrow_mut().world.backdrop_filter_mut(id) {
                    set(&mut b, v);
                }
                Ok(())
            }),
        )?;
    }
    put(
        &table,
        "GetTint",
        scope.create_function(|_, id: u32| {
            let scene = scene.borrow();
            let t = scene
                .world
                .backdrop_filter(id)
                .map_or(Vec4::ZERO, |b| b.tint);
            Ok((t.x, t.y, t.z, t.w))
        }),
    )?;
    put(
        &table,
        "SetTint",
        scope.create_function(|_, (id, r, g, b, a): (u32, f32, f32, f32, f32)| {
            if let Some(mut f) = scene.borrow_mut().world.backdrop_filter_mut(id) {
                ops::set_tint(&mut f, Vec4::new(r, g, b, a));
            }
            Ok(())
        }),
    )?;
    lua.globals()
        .set("BackdropFilter", table)
        .map_err(|e| e.to_string())
}
