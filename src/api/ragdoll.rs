//! src/api/ragdoll.rs — `Ragdoll` namespace (#466).
//!
//! `Build` is the inspector's *Build Ragdoll* button (Unity's Ragdoll Wizard):
//! a `Rigidbody` and a `Joint` on each hitbox bone, see `scene::skeleton::ragdoll`.
//! `Enable` / `Disable` hand a character's bones from the Animator to physics
//! and back; `Enable` seeds each body with the velocity its animation gave it,
//! read off the live physics world. Each takes the skinned entity or any ancestor
//! of it — the `root` a raycast returns works.

use std::cell::RefCell;

use mlua::{Lua, Table};

use super::{put, Reg};
use crate::physics::PhysicsWorld;
use crate::scene::skeleton::RagdollOptions;
use crate::scene::Scene;

/// Register the `Ragdoll` namespace onto `lua`.
pub fn register<'scope>(
    lua: &Lua,
    scope: &'scope mlua::Scope<'scope, '_>,
    scene: &'scope RefCell<Scene>,
    physics: &'scope RefCell<Option<PhysicsWorld>>,
) -> Reg {
    let t = lua.create_table().map_err(|e| e.to_string())?;
    put(
        &t,
        "Build",
        scope.create_function(|lua, (id, opts): (u32, Option<Table>)| {
            let mut options = RagdollOptions::default();
            if let Some(mass) = opts.map(|o| o.get::<Option<f32>>("mass")).transpose()? {
                options.mass = mass.unwrap_or(options.mass).max(0.001);
            }
            let made = scene
                .borrow_mut()
                .build_ragdoll(id, &options)
                .map_err(mlua::Error::RuntimeError)?;
            let out = lua.create_table()?;
            for (bone_name, bone) in made {
                out.set(bone_name, bone)?;
            }
            Ok(out)
        }),
    )?;
    for (name, on) in [("Enable", true), ("Disable", false)] {
        let f = scope.create_function(move |_, id: u32| {
            let velocity = |bone: u32| physics.borrow().as_ref()?.body_velocity(bone);
            scene
                .borrow_mut()
                .set_ragdoll(id, on, velocity)
                .map_err(mlua::Error::RuntimeError)
        });
        put(&t, name, f)?;
    }
    put(
        &t,
        "IsEnabled",
        scope.create_function(|_, id: u32| Ok(scene.borrow().ragdoll_enabled(id))),
    )?;
    lua.globals().set("Ragdoll", t).map_err(|e| e.to_string())
}
