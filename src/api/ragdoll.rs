//! src/api/ragdoll.rs — `Ragdoll` namespace (#466).
//!
//! `Build` is the inspector's *Build Ragdoll* button (Unity's Ragdoll Wizard):
//! a `Rigidbody` and a `Joint` on each hitbox bone, see `scene::skeleton::ragdoll`.
//! `Enable` / `Disable` hand a character's bones from the Animator to physics
//! and back; `Enable` seeds each body with the velocity its animation gave it,
//! read off the live physics world. Each takes the skinned entity or any ancestor
//! of it — the `root` a raycast returns works.
//!
//! The partial ragdoll (#1895, `scene::skeleton::ragdoll::partial`):
//! `SetSimulated`, `SetDrive` and `SetBlendWeight` act on the branch at a bone,
//! and `HitReact` is the one call a game makes when a shot lands.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;

use glam::Vec3;
use mlua::{Lua, Table};

use super::{put, Reg};
use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use crate::physics::PhysicsWorld;
use crate::scene::skeleton::{Drive, HitReact, RagdollOptions};
use crate::scene::Scene;

/// `(id, bone, ix, iy, iz, px, py, pz, opts)`: `HitReact`'s arguments.
type HitArgs = (
    EntityArg,
    EntityArg,
    f32,
    f32,
    f32,
    f32,
    f32,
    f32,
    Option<Table>,
);

/// Register the `Ragdoll` namespace onto `lua`.
pub fn register(lua: &Lua, scene: ApiCell<Scene>, physics: ApiCell<Option<PhysicsWorld>>) -> Reg {
    let t = lua.create_table().map_err(|e| e.to_string())?;
    put(
        &t,
        "Build",
        lua.create_function(move |lua, (id, opts): (EntityArg, Option<Table>)| {
            let scene = &*scene.get(lua)?;
            let id = id.live("Ragdoll", "Build")?;
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
        let f = lua.create_function(move |lua, id: EntityArg| {
            physics.with(lua, |physics| {
                let id = id.live("Ragdoll", name)?;
                let velocity = |bone: u32| physics.borrow().as_ref()?.body_velocity(bone);
                scene
                    .get(lua)?
                    .borrow_mut()
                    .set_ragdoll(id, on, velocity)
                    .map_err(mlua::Error::RuntimeError)
            })
        });
        put(&t, name, f)?;
    }
    put(
        &t,
        "IsEnabled",
        lua.create_function(move |lua, id: EntityArg| {
            Ok(scene.get(lua)?.borrow().ragdoll_enabled(id.read()))
        }),
    )?;
    register_partial(lua, &t, scene, physics)?;
    lua.globals().set("Ragdoll", t).map_err(|e| e.to_string())
}

/// `SetSimulated`, `SetDrive`, `SetBlendWeight` and `HitReact` (#1895).
fn register_partial(
    lua: &Lua,
    t: &Table,
    scene: ApiCell<Scene>,
    physics: ApiCell<Option<PhysicsWorld>>,
) -> Reg {
    let f = lua.create_function(move |lua, (id, bone, on): (EntityArg, EntityArg, bool)| {
        let id = id.live("Ragdoll", "SetSimulated")?;
        physics.with(lua, |physics| {
            let velocity = |b: u32| physics.borrow().as_ref()?.body_velocity(b);
            let scene = scene.get(lua)?;
            let n = scene
                .borrow_mut()
                .set_simulated(id, bone.read(), on, &velocity);
            Ok(n)
        })
    });
    put(t, "SetSimulated", f)?;
    let f = lua.create_function(
        move |lua, (id, bone, opts): (EntityArg, EntityArg, Table)| {
            let id = id.live("Ragdoll", "SetDrive")?;
            let given = [opts.get("strength")?, opts.get("damping")?];
            Ok(scene
                .get(lua)?
                .borrow_mut()
                .set_drive(id, bone.read(), given))
        },
    );
    put(t, "SetDrive", f)?;
    let f = lua.create_function(move |lua, (id, bone, w): (EntityArg, EntityArg, f32)| {
        let id = id.live("Ragdoll", "SetBlendWeight")?;
        Ok(scene
            .get(lua)?
            .borrow_mut()
            .set_blend_weight(id, bone.read(), w))
    });
    put(t, "SetBlendWeight", f)?;
    register_hit_react(lua, t, scene, physics)
}

/// `HitReact`: one warning per entity without a ragdoll there.
fn register_hit_react(
    lua: &Lua,
    t: &Table,
    scene: ApiCell<Scene>,
    physics: ApiCell<Option<PhysicsWorld>>,
) -> Reg {
    let warned = Rc::new(RefCell::new(BTreeSet::new()));
    let f = lua.create_function(move |lua, (id, bone, ix, iy, iz, px, py, pz, opts): HitArgs| {
        let id = id.live("Ragdoll", "HitReact")?;
        let opts = react_from(opts.as_ref())?;
        physics.with(lua, |physics| {
            let velocity = |b: u32| physics.borrow().as_ref()?.body_velocity(b);
            let scene = scene.get(lua)?;
            let point = Vec3::new(px, py, pz);
            let hit = scene.borrow_mut().hit_react(id, (bone.read(), point), &opts, &velocity);
            let Some(hit) = hit else {
                if warned.borrow_mut().insert(id) {
                    log::warn!(
                        "[Ragdoll] HitReact: entity {id} has no ragdoll at that bone (Ragdoll.Build it first)"
                    );
                }
                return Ok(false);
            };
            let impulse = Vec3::new(ix, iy, iz);
            if impulse.is_finite() && point.is_finite() {
                crate::api::physics::impulse::apply(&scene, physics, hit, impulse, point);
            }
            Ok(true)
        })
    });
    put(t, "HitReact", f)
}

/// `base` with the `{ strength, damping }` an options table gives ([`Drive::merged`]).
fn drive_from(opts: Option<&Table>, base: Drive) -> mlua::Result<Drive> {
    let Some(o) = opts else {
        return Ok(base);
    };
    Ok(base.merged(o.get("strength")?, o.get("damping")?))
}

/// `HitReact`'s options: `{ duration, strength, damping, up_to }`.
fn react_from(opts: Option<&Table>) -> mlua::Result<HitReact> {
    let base = HitReact::default();
    let Some(o) = opts else {
        return Ok(base);
    };
    let duration = o.get::<Option<f32>>("duration")?.unwrap_or(base.duration);
    Ok(HitReact {
        duration: if duration.is_nan() {
            base.duration
        } else {
            duration.max(0.0)
        },
        drive: drive_from(Some(o), base.drive)?,
        up_to: o.get::<Option<EntityArg>>("up_to")?.and_then(|b| b.get()),
    })
}
