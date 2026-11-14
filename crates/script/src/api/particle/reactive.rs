//! crates/script/src/api/particle/reactive.rs — the `Particles` TAA reactivity pair (#1845):
//! `SetReactive` / `GetReactive`. The setter is a field write routed through the
//! shared `scene::authoring::particles` op the inspector calls.

use super::with_emitter;
use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use crate::api::{put, Reg};
use rusty_components::TaaReactive;
use rusty_scene::authoring::particles as particle_ops;
use rusty_scene::Scene;

/// Register the reactivity pair onto the `Particles` table.
pub(super) fn register(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    // `SetReactive(id, "Auto"|"Off"|"Full")`; an unknown name is an error.
    put(
        table,
        "SetReactive",
        lua.create_function(move |lua, (id, name): (EntityArg, String)| {
            let id = id.live("Particles", "SetReactive")?;
            let reactive = TaaReactive::parse(&name).ok_or_else(|| {
                mlua::Error::RuntimeError(format!(
                    "SetReactive: unknown reactivity {name:?} (want one of {})",
                    TaaReactive::names()
                ))
            })?;
            with_emitter(&*scene.get(lua)?, id, |p| {
                particle_ops::set_reactive(p, reactive)
            });
            Ok(())
        }),
    )?;
    put(
        table,
        "GetReactive",
        lua.create_function(move |lua, id: EntityArg| {
            let scene = &*scene.get(lua)?;
            let scene = scene.borrow();
            let reactive = scene
                .world
                .particles(id.read())
                .map(|p| p.render.reactive.name());
            Ok(reactive)
        }),
    )
}
