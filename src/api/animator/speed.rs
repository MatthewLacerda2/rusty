//! src/api/animator/speed.rs — the `Animator` playback-speed surface (#1342).
//!
//! `SetSpeed` / `GetSpeed` over the component's own `speed` multiplier, Unity's
//! `animator.speed`: it scales every layer's playhead on top of each graph
//! node's own speed. Set from `Update` it takes effect in the same fixed step
//! (`update_scripts` runs before `animate`); from `LateUpdate`, the next one. The
//! write goes through `authoring::animator::set_speed`, the same op the
//! inspector's Animator card uses, so both accept the same values: any finite
//! number (0 holds the pose, negative runs the playhead backwards). That op keeps
//! the old speed on NaN or infinity; the binding also raises a Lua error, so a
//! script learns its value was refused.

use std::cell::RefCell;

use super::{put, Reg};
use crate::scene::authoring::animator as animator_ops;
use crate::scene::Scene;

/// Register `SetSpeed` / `GetSpeed` onto the `Animator` table.
pub(super) fn register<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    put(
        table,
        "SetSpeed",
        scope.create_function(|_, (id, speed): (u32, f32)| {
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
        scope.create_function(|_, id: u32| Ok(scene.borrow().world.animator(id).map(|a| a.speed))),
    )
}
