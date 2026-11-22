use super::with_emitter;
use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use crate::api::{put, Reg};
use rusty_components::TaaReactive;
use rusty_edit::authoring::particles as particle_ops;
use rusty_scene::Scene;

pub(super) fn register(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
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
