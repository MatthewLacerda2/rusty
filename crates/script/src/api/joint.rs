//! crates/script/src/api/joint.rs — `Joint` namespace (#449).
//!
//! Get/Set over an entity's `JointComponent`: its kind, the connected body, the
//! anchors and axes, the limits, the break thresholds, the collision flag and the
//! drive (#1895).
//! Every setter routes through the shared `scene::authoring::physics::joint` ops the
//! inspector card uses. Getters return a neutral default without a Joint; setters
//! are then no-ops. The kind travels as its name (`"Fixed"`, `"Hinge"`, `"Ball"`,
//! case-insensitive; an unknown name is ignored). Attach one with
//! `Scene.AddComponent(id, "Joint")`; a broken joint's component is gone, so
//! `Scene.HasComponent(id, "Joint")` turns false and `OnJointBreak` fires.

use std::cell::RefCell;

use glam::Vec3;
use mlua::{Lua, Table};

use super::{put, Reg};
use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use rusty_components::{JointComponent, JointKind};
use rusty_scene::authoring::physics::joint as ops;
use rusty_scene::Scene;

/// A field's reader and its shared-op writer.
type Field<T> = (fn(&JointComponent) -> T, fn(&mut JointComponent, T));
/// `(id, min, max)`.
type IdRange = (EntityArg, f32, f32);

/// Register the `Joint` namespace onto `lua`.
pub fn register(lua: &Lua, scene: ApiCell<Scene>) -> Reg {
    let t = lua.create_table().map_err(|e| e.to_string())?;
    register_shape(lua, &t, scene)?;
    register_limits(lua, &t, scene)?;
    lua.globals().set("Joint", t).map_err(|e| e.to_string())
}

/// `id`'s Joint, cloned, or `None` without one.
fn get(scene: &RefCell<Scene>, id: u32) -> Option<JointComponent> {
    scene.borrow().world.joint(id).map(|j| j.clone())
}

/// Apply `f` to `id`'s Joint, when it has one.
fn set(scene: &RefCell<Scene>, id: u32, f: &dyn Fn(&mut JointComponent)) {
    if let Some(mut j) = scene.borrow_mut().world.joint_mut(id) {
        f(&mut j);
    }
}

/// `Get<name>` / `Set<name>` over one `f32` field (`0` without a Joint).
fn f32_pair(
    lua: &mlua::Lua,
    t: &Table,
    scene: ApiCell<Scene>,
    name: &str,
    (read, write): Field<f32>,
) -> Reg {
    let f = lua.create_function(move |lua, id: EntityArg| {
        Ok(get(&*scene.get(lua)?, id.read()).map_or(0.0, |j| read(&j)))
    });
    put(t, &format!("Get{name}"), f)?;
    let verb = format!("Set{name}");
    let f = lua.create_function(move |lua, (id, v): (EntityArg, f32)| {
        let id = id.live("Joint", &verb)?;
        set(&*scene.get(lua)?, id, &|j| write(j, v));
        Ok(())
    });
    put(t, &format!("Set{name}"), f)
}

/// `Get<name>` / `Set<name>` over one `bool` field (`false` without a Joint).
fn bool_pair(
    lua: &mlua::Lua,
    t: &Table,
    scene: ApiCell<Scene>,
    name: &str,
    (read, write): Field<bool>,
) -> Reg {
    let f = lua.create_function(move |lua, id: EntityArg| {
        Ok(get(&*scene.get(lua)?, id.read()).is_some_and(|j| read(&j)))
    });
    put(t, &format!("Get{name}"), f)?;
    let verb = format!("Set{name}");
    let f = lua.create_function(move |lua, (id, v): (EntityArg, bool)| {
        let id = id.live("Joint", &verb)?;
        set(&*scene.get(lua)?, id, &|j| write(j, v));
        Ok(())
    });
    put(t, &format!("Set{name}"), f)
}

/// `Get<name>` → `x, y, z` / `Set<name>(id, x, y, z)` over one `Vec3` field.
fn vec3_pair(
    lua: &mlua::Lua,
    t: &Table,
    scene: ApiCell<Scene>,
    name: &str,
    (read, write): Field<Vec3>,
) -> Reg {
    let f = lua.create_function(move |lua, id: EntityArg| {
        let v = get(&*scene.get(lua)?, id.read()).map_or(Vec3::ZERO, |j| read(&j));
        Ok((v.x, v.y, v.z))
    });
    put(t, &format!("Get{name}"), f)?;
    let verb = format!("Set{name}");
    let f = lua.create_function(move |lua, (id, x, y, z): (EntityArg, f32, f32, f32)| {
        let id = id.live("Joint", &verb)?;
        set(&*scene.get(lua)?, id, &|j| write(j, Vec3::new(x, y, z)));
        Ok(())
    });
    put(t, &format!("Set{name}"), f)
}

/// `Get/SetKind`, `Get/SetConnectedBody`, the anchors, the axes and
/// `Get/SetEnableCollision`.
fn register_shape(lua: &mlua::Lua, t: &Table, scene: ApiCell<Scene>) -> Reg {
    let f = lua.create_function(move |lua, id: EntityArg| {
        Ok(get(&*scene.get(lua)?, id.read()).map(|j| j.kind.name()))
    });
    put(t, "GetKind", f)?;
    let f = lua.create_function(move |lua, (id, name): (EntityArg, String)| {
        let id = id.live("Joint", "SetKind")?;
        if let Some(k) = JointKind::parse(&name) {
            set(&*scene.get(lua)?, id, &|j| ops::set_kind(j, k));
        }
        Ok(())
    });
    put(t, "SetKind", f)?;
    let f = lua.create_function(move |lua, id: EntityArg| {
        Ok(get(&*scene.get(lua)?, id.read()).and_then(|j| j.connected_body))
    });
    put(t, "GetConnectedBody", f)?;
    let f = lua.create_function(move |lua, (id, body): (EntityArg, EntityArg)| {
        let id = id.live("Joint", "SetConnectedBody")?;
        let body = body.get();
        set(&*scene.get(lua)?, id, &|j| ops::set_connected_body(j, body));
        Ok(())
    });
    put(t, "SetConnectedBody", f)?;
    vec3_pair(lua, t, scene, "Anchor", (|j| j.anchor, ops::set_anchor))?;
    let connected: Field<Vec3> = (|j| j.connected_anchor, ops::set_connected_anchor);
    vec3_pair(lua, t, scene, "ConnectedAnchor", connected)?;
    let auto: Field<bool> = (
        |j| j.auto_configure_connected_anchor,
        ops::set_auto_configure_connected_anchor,
    );
    bool_pair(lua, t, scene, "AutoConfigureConnectedAnchor", auto)?;
    vec3_pair(lua, t, scene, "Axis", (|j| j.axis, ops::set_axis))?;
    let swing_axis: Field<Vec3> = (|j| j.swing_axis, ops::set_swing_axis);
    vec3_pair(lua, t, scene, "SwingAxis", swing_axis)?;
    let collide: Field<bool> = (|j| j.enable_collision, ops::set_enable_collision);
    bool_pair(lua, t, scene, "EnableCollision", collide)
}

/// `Get/SetUseLimits`, `Get/SetLimits` (min, max degrees), `Get/SetSwingLimit`,
/// `Get/SetSwing2Limit`, `Get/SetBreakForce`, `Get/SetBreakTorque`,
/// `Get/SetDriveStrength`, `Get/SetDriveDamping`.
fn register_limits(lua: &mlua::Lua, t: &Table, scene: ApiCell<Scene>) -> Reg {
    bool_pair(
        lua,
        t,
        scene,
        "UseLimits",
        (|j| j.use_limits, ops::set_use_limits),
    )?;
    let f = lua.create_function(move |lua, id: EntityArg| {
        let l = get(&*scene.get(lua)?, id.read()).map_or_else(Default::default, |j| j.limits);
        Ok((l.x, l.y))
    });
    put(t, "GetLimits", f)?;
    let f = lua.create_function(move |lua, (id, min, max): IdRange| {
        let id = id.live("Joint", "SetLimits")?;
        set(&*scene.get(lua)?, id, &|j| ops::set_limits(j, min, max));
        Ok(())
    });
    put(t, "SetLimits", f)?;
    let swing: Field<f32> = (|j| j.swing_limit, ops::set_swing_limit);
    f32_pair(lua, t, scene, "SwingLimit", swing)?;
    let swing2: Field<f32> = (|j| j.swing2_limit, ops::set_swing2_limit);
    f32_pair(lua, t, scene, "Swing2Limit", swing2)?;
    let force: Field<f32> = (|j| j.break_force, ops::set_break_force);
    f32_pair(lua, t, scene, "BreakForce", force)?;
    let torque: Field<f32> = (|j| j.break_torque, ops::set_break_torque);
    f32_pair(lua, t, scene, "BreakTorque", torque)?;
    let strength: Field<f32> = (|j| j.drive_strength, ops::set_drive_strength);
    f32_pair(lua, t, scene, "DriveStrength", strength)?;
    let damping: Field<f32> = (|j| j.drive_damping, ops::set_drive_damping);
    f32_pair(lua, t, scene, "DriveDamping", damping)
}
