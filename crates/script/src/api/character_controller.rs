use std::cell::RefCell;

use glam::Vec3;
use mlua::{Lua, Table};

use super::{put, Reg};
use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use crate::scripting::dispatch_controller_hits;
use rusty_components::physics::character_controller::{
    COLLIDED_ABOVE, COLLIDED_BELOW, COLLIDED_SIDES,
};
use rusty_components::CharacterControllerComponent as Cc;
use rusty_edit::authoring::physics::character_controller as ops;
use rusty_physics::PhysicsWorld;
use rusty_scene::Scene;

type Field<T> = (fn(&Cc) -> T, fn(&mut Cc, T));
type Flags = (bool, bool, bool);

pub fn register(lua: &Lua, scene: ApiCell<Scene>, physics: ApiCell<Option<PhysicsWorld>>) -> Reg {
    let t = lua.create_table().map_err(|e| e.to_string())?;
    register_motion(lua, &t, scene, physics)?;
    register_shape(lua, &t, scene)?;
    lua.globals()
        .set("CharacterController", t)
        .map_err(|e| e.to_string())
}

fn get(scene: &RefCell<Scene>, id: u32) -> Option<Cc> {
    scene
        .borrow()
        .world
        .character_controller(id)
        .map(|c| c.clone())
}

fn flags(mask: u8) -> Flags {
    (
        mask & COLLIDED_BELOW != 0,
        mask & COLLIDED_SIDES != 0,
        mask & COLLIDED_ABOVE != 0,
    )
}

fn register_motion(
    lua: &mlua::Lua,
    t: &Table,
    scene: ApiCell<Scene>,
    physics: ApiCell<Option<PhysicsWorld>>,
) -> Reg {
    let f = lua.create_function(move |lua, (id, x, y, z): (EntityArg, f32, f32, f32)| {
        let scene = &*scene.get(lua)?;
        let physics = &*physics.get(lua)?;
        let id = id.live("CharacterController", "Move")?;
        let moved = {
            let world = physics.borrow();
            let mut scene = scene.borrow_mut();
            rusty_physics::move_character(world.as_ref(), &mut scene, id, Vec3::new(x, y, z))
        };
        let Some(moved) = moved else {
            return Ok(flags(0));
        };
        dispatch_controller_hits(lua, scene, id, &moved.hits)?;
        Ok(flags(moved.flags))
    });
    put(t, "Move", f)?;
    let f = lua.create_function(move |lua, id: EntityArg| {
        Ok(get(&*scene.get(lua)?, id.read()).is_some_and(|c| c.is_grounded))
    });
    put(t, "IsGrounded", f)?;
    let f = lua.create_function(move |lua, id: EntityArg| {
        Ok(flags(
            get(&*scene.get(lua)?, id.read()).map_or(0, |c| c.collision_flags),
        ))
    });
    put(t, "GetCollisionFlags", f)?;
    let f = lua.create_function(move |lua, id: EntityArg| {
        let n = get(&*scene.get(lua)?, id.read()).map_or(Vec3::Y, |c| c.ground_normal);
        Ok((n.x, n.y, n.z))
    });
    put(t, "GetGroundNormal", f)?;
    let f = lua.create_function(move |lua, (id, height): (EntityArg, f32)| {
        physics.with(lua, |physics| {
            let world = physics.borrow();
            Ok(rusty_physics::can_stand(
                world.as_ref(),
                &scene.get(lua)?.borrow(),
                id.read(),
                height,
            ))
        })
    });
    put(t, "CanStand", f)
}

fn f32_pair(
    lua: &mlua::Lua,
    t: &Table,
    scene: ApiCell<Scene>,
    name: &str,
    (read, write): Field<f32>,
) -> Reg {
    let f = lua.create_function(move |lua, id: EntityArg| {
        Ok(get(&*scene.get(lua)?, id.read()).map_or(0.0, |c| read(&c)))
    });
    put(t, &format!("Get{name}"), f)?;
    let verb = format!("Set{name}");
    let f = lua.create_function(move |lua, (id, v): (EntityArg, f32)| {
        let id = id.live("CharacterController", &verb)?;
        if let Some(mut c) = scene
            .get(lua)?
            .borrow_mut()
            .world
            .character_controller_mut(id)
        {
            write(&mut c, v);
        }
        Ok(())
    });
    put(t, &format!("Set{name}"), f)
}

fn register_shape(lua: &mlua::Lua, t: &Table, scene: ApiCell<Scene>) -> Reg {
    f32_pair(lua, t, scene, "Height", (|c| c.height, ops::set_height))?;
    f32_pair(lua, t, scene, "Radius", (|c| c.radius, ops::set_radius))?;
    let step: Field<f32> = (|c| c.step_offset, ops::set_step_offset);
    f32_pair(lua, t, scene, "StepOffset", step)?;
    let slope: Field<f32> = (|c| c.slope_limit, ops::set_slope_limit);
    f32_pair(lua, t, scene, "SlopeLimit", slope)?;
    let skin: Field<f32> = (|c| c.skin_width, ops::set_skin_width);
    f32_pair(lua, t, scene, "SkinWidth", skin)?;
    let min_move: Field<f32> = (|c| c.min_move_distance, ops::set_min_move_distance);
    f32_pair(lua, t, scene, "MinMoveDistance", min_move)?;
    let f = lua.create_function(move |lua, id: EntityArg| {
        let v = get(&*scene.get(lua)?, id.read()).map_or(Vec3::ZERO, |c| c.center);
        Ok((v.x, v.y, v.z))
    });
    put(t, "GetCenter", f)?;
    let f = lua.create_function(move |lua, (id, x, y, z): (EntityArg, f32, f32, f32)| {
        let id = id.live("CharacterController", "SetCenter")?;
        if let Some(mut c) = scene
            .get(lua)?
            .borrow_mut()
            .world
            .character_controller_mut(id)
        {
            ops::set_center(&mut c, Vec3::new(x, y, z));
        }
        Ok(())
    });
    put(t, "SetCenter", f)
}
