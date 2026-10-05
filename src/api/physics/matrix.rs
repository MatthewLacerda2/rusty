//! src/api/physics/matrix.rs — the Layer Collision Matrix from script (#827).
//!
//! Unity's `Physics.IgnoreLayerCollision` / `GetIgnoreLayerCollision` over the
//! scene's `CollisionMatrix` — the same `set_collision` the editor's matrix grid
//! writes. A change reaches live colliders before the next physics step: the world
//! re-derives every collider's groups from the matrix each step (`sync_layers`),
//! and the character sweep reads it per move, so it applies immediately, as in
//! Unity.

use std::cell::RefCell;

use crate::api::layers::check_index;
use crate::api::{put, Reg};
use crate::scene::Scene;

/// Register `IgnoreLayerCollision` / `GetIgnoreLayerCollision` onto `Physics`.
pub fn register<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    put(
        table,
        "IgnoreLayerCollision",
        scope.create_function(|_, (a, b, ignore): (u8, u8, Option<bool>)| {
            check_index(a)?;
            check_index(b)?;
            let collide = !ignore.unwrap_or(true);
            scene
                .borrow_mut()
                .collision_matrix
                .set_collision(a, b, collide);
            Ok(())
        }),
    )?;

    put(
        table,
        "GetIgnoreLayerCollision",
        scope.create_function(|_, (a, b): (u8, u8)| {
            check_index(a)?;
            check_index(b)?;
            Ok(!scene.borrow().collision_matrix.can_collide(a, b))
        }),
    )
}
