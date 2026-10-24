//! src/api/physics/body.rs — `Physics.GetMass` / `SetMass` / `GetUseGravity` /
//! `SetUseGravity` / `IsTrigger` / `SetTrigger` (#1365, #1368).
//!
//! The body's authorable switches, over the same `scene::authoring` ops the
//! inspector's Rigidbody and Collider cards write through, so a script's mass is
//! clamped exactly as the card's is. Unity: `Rigidbody.mass`, `Rigidbody.useGravity`
//! and `Collider.isTrigger`. Getters answer a neutral default without the
//! component; setters are then no-ops.

use std::cell::RefCell;

use mlua::Table;

use super::super::{put, Reg};
use crate::api::entity_arg::EntityArg;
use crate::scene::authoring::{collider as collider_ops, rigidbody as rb_ops};
use crate::scene::Scene;

/// Register the mass, gravity and trigger accessors onto the `Physics` table.
pub(super) fn register<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    let f = scope.create_function(|_, id: EntityArg| {
        let id = id.read();
        Ok(scene.borrow().world.rigidbody(id).map_or(0.0, |rb| rb.mass))
    });
    put(table, "GetMass", f)?;
    let f = scope.create_function(|_, (id, mass): (EntityArg, f32)| {
        let id = id.live("Physics", "SetMass")?;
        if let Some(mut rb) = scene.borrow_mut().world.rigidbody_mut(id) {
            rb_ops::set_mass(&mut rb, mass);
        }
        Ok(())
    });
    put(table, "SetMass", f)?;
    let f = scope.create_function(|_, id: EntityArg| {
        let id = id.read();
        let world = &scene.borrow().world;
        Ok(world.rigidbody(id).is_some_and(|rb| rb.use_gravity))
    });
    put(table, "GetUseGravity", f)?;
    let f = scope.create_function(|_, (id, on): (EntityArg, bool)| {
        let id = id.live("Physics", "SetUseGravity")?;
        if let Some(mut rb) = scene.borrow_mut().world.rigidbody_mut(id) {
            rb_ops::set_use_gravity(&mut rb, on);
        }
        Ok(())
    });
    put(table, "SetUseGravity", f)?;
    let f = scope.create_function(|_, id: EntityArg| {
        let id = id.read();
        let world = &scene.borrow().world;
        Ok(world.collider(id).is_some_and(|c| c.is_trigger))
    });
    put(table, "IsTrigger", f)?;
    let f = scope.create_function(|_, (id, on): (EntityArg, bool)| {
        let id = id.live("Physics", "SetTrigger")?;
        if let Some(mut c) = scene.borrow_mut().world.collider_mut(id) {
            collider_ops::set_trigger(&mut c, on);
        }
        Ok(())
    });
    put(table, "SetTrigger", f)
}
