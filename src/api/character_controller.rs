//! src/api/character_controller.rs — `CharacterController` namespace (#451).
//!
//! Unity's `CharacterController`: `Move` sweeps the entity's capsule by a
//! displacement with collide-and-slide against the live physics world and
//! returns what it touched; the getters read back the grounded state that move
//! left; the size and tuning setters route through the shared
//! `scene::authoring::character_controller` ops the inspector card uses.
//! Getters return a neutral default without a controller; setters and `Move` are
//! then no-ops. Attach one with `Scene.AddComponent(id, "CharacterController")`.

use std::cell::RefCell;

use glam::Vec3;
use mlua::{Lua, Table};

use super::{put, Reg};
use crate::components::character_controller::{COLLIDED_ABOVE, COLLIDED_BELOW, COLLIDED_SIDES};
use crate::components::CharacterControllerComponent as Cc;
use crate::physics::{self, PhysicsWorld};
use crate::scene::authoring::character_controller as ops;
use crate::scene::Scene;

type SceneCell<'s> = &'s RefCell<Scene>;
type PhysicsCell<'s> = &'s RefCell<Option<PhysicsWorld>>;
/// A field's reader and its shared-op writer.
type Field<T> = (fn(&Cc) -> T, fn(&mut Cc, T));
/// `below, sides, above`.
type Flags = (bool, bool, bool);

/// Register the `CharacterController` namespace onto `lua`.
pub fn register<'scope>(
    lua: &Lua,
    scope: &'scope mlua::Scope<'scope, '_>,
    scene: &'scope RefCell<Scene>,
    physics: &'scope RefCell<Option<PhysicsWorld>>,
) -> Reg {
    let t = lua.create_table().map_err(|e| e.to_string())?;
    register_motion(scope, &t, scene, physics)?;
    register_shape(scope, &t, scene)?;
    lua.globals()
        .set("CharacterController", t)
        .map_err(|e| e.to_string())
}

/// `id`'s controller, cloned, or `None` without one.
fn get(scene: SceneCell, id: u32) -> Option<Cc> {
    scene
        .borrow()
        .world
        .character_controller(id)
        .map(|c| c.clone())
}

/// Split a `COLLIDED_*` mask into `below, sides, above`.
fn flags(mask: u8) -> Flags {
    (
        mask & COLLIDED_BELOW != 0,
        mask & COLLIDED_SIDES != 0,
        mask & COLLIDED_ABOVE != 0,
    )
}

/// `Move`, `IsGrounded`, `GetCollisionFlags`, `GetGroundNormal`, `CanStand`.
fn register_motion<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    t: &Table,
    scene: SceneCell<'scope>,
    physics: PhysicsCell<'scope>,
) -> Reg {
    let f = scope.create_function(move |_, (id, x, y, z): (u32, f32, f32, f32)| {
        let world = physics.borrow();
        let mut scene = scene.borrow_mut();
        let hit = physics::move_character(world.as_ref(), &mut scene, id, Vec3::new(x, y, z));
        Ok(flags(hit.map_or(0, |m| m.flags)))
    });
    put(t, "Move", f)?;
    let f =
        scope.create_function(move |_, id: u32| Ok(get(scene, id).is_some_and(|c| c.is_grounded)));
    put(t, "IsGrounded", f)?;
    let f = scope.create_function(move |_, id: u32| {
        Ok(flags(get(scene, id).map_or(0, |c| c.collision_flags)))
    });
    put(t, "GetCollisionFlags", f)?;
    let f = scope.create_function(move |_, id: u32| {
        let n = get(scene, id).map_or(Vec3::Y, |c| c.ground_normal);
        Ok((n.x, n.y, n.z))
    });
    put(t, "GetGroundNormal", f)?;
    let f = scope.create_function(move |_, (id, height): (u32, f32)| {
        let world = physics.borrow();
        Ok(physics::can_stand(
            world.as_ref(),
            &scene.borrow(),
            id,
            height,
        ))
    });
    put(t, "CanStand", f)
}

/// `Get<name>` / `Set<name>` over one `f32` field (`0` without a controller).
fn f32_pair<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    t: &Table,
    scene: SceneCell<'scope>,
    name: &str,
    (read, write): Field<f32>,
) -> Reg {
    let f = scope.create_function(move |_, id: u32| Ok(get(scene, id).map_or(0.0, |c| read(&c))));
    put(t, &format!("Get{name}"), f)?;
    let f = scope.create_function(move |_, (id, v): (u32, f32)| {
        if let Some(mut c) = scene.borrow_mut().world.character_controller_mut(id) {
            write(&mut c, v);
        }
        Ok(())
    });
    put(t, &format!("Set{name}"), f)
}

/// The capsule (`Height`, `Radius`, `Center`) and its tuning (`StepOffset`,
/// `SlopeLimit`, `SkinWidth`, `MinMoveDistance`).
fn register_shape<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    t: &Table,
    scene: SceneCell<'scope>,
) -> Reg {
    f32_pair(scope, t, scene, "Height", (|c| c.height, ops::set_height))?;
    f32_pair(scope, t, scene, "Radius", (|c| c.radius, ops::set_radius))?;
    let step: Field<f32> = (|c| c.step_offset, ops::set_step_offset);
    f32_pair(scope, t, scene, "StepOffset", step)?;
    let slope: Field<f32> = (|c| c.slope_limit, ops::set_slope_limit);
    f32_pair(scope, t, scene, "SlopeLimit", slope)?;
    let skin: Field<f32> = (|c| c.skin_width, ops::set_skin_width);
    f32_pair(scope, t, scene, "SkinWidth", skin)?;
    let min_move: Field<f32> = (|c| c.min_move_distance, ops::set_min_move_distance);
    f32_pair(scope, t, scene, "MinMoveDistance", min_move)?;
    let f = scope.create_function(move |_, id: u32| {
        let v = get(scene, id).map_or(Vec3::ZERO, |c| c.center);
        Ok((v.x, v.y, v.z))
    });
    put(t, "GetCenter", f)?;
    let f = scope.create_function(move |_, (id, x, y, z): (u32, f32, f32, f32)| {
        if let Some(mut c) = scene.borrow_mut().world.character_controller_mut(id) {
            ops::set_center(&mut c, Vec3::new(x, y, z));
        }
        Ok(())
    });
    put(t, "SetCenter", f)
}
