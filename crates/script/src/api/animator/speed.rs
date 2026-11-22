use super::{put, Reg};
use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use rusty_edit::authoring::animator as animator_ops;
use rusty_scene::Scene;

pub(super) fn register(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "SetSpeed",
        lua.create_function(move |lua, (id, speed): (EntityArg, f32)| {
            let scene = &*scene.get(lua)?;
            let id = id.live("Animator", "SetSpeed")?;
            if !speed.is_finite() {
                return Err(mlua::Error::RuntimeError(format!(
                    "Animator.SetSpeed: speed must be a finite number, got {speed}"
                )));
            }
            let mut scene = scene.borrow_mut();
            if let Some(mut c) = scene.world.animator_mut(id) {
                animator_ops::set_speed(&mut c, speed);
            }
            Ok(())
        }),
    )?;

    put(
        table,
        "GetSpeed",
        lua.create_function(move |lua, id: EntityArg| {
            Ok(scene
                .get(lua)?
                .borrow()
                .world
                .animator(id.read())
                .map(|a| a.speed))
        }),
    )
}
