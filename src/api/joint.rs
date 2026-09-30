//! src/api/joint.rs — `Joint` namespace (#449).
//!
//! Get/Set over an entity's `JointComponent`: its kind, the connected body, the
//! anchors and axis, the limits, the break thresholds and the collision flag.
//! Every setter routes through the shared `scene::authoring::joint` ops the
//! inspector card uses. Getters return a neutral default without a Joint; setters
//! are then no-ops. The kind travels as its name (`"Fixed"`, `"Hinge"`, `"Ball"`,
//! case-insensitive; an unknown name is ignored). Attach one with
//! `Scene.AddComponent(id, "Joint")`; a broken joint's component is gone, so
//! `Scene.HasComponent(id, "Joint")` turns false and `OnJointBreak` fires.

use std::cell::RefCell;

use glam::Vec3;
use mlua::{Lua, Table};

use super::{put, Reg};
use crate::components::{JointComponent, JointKind};
use crate::scene::authoring::joint as ops;
use crate::scene::Scene;

type SceneCell<'s> = &'s RefCell<Scene>;
/// A field's reader and its shared-op writer.
type Field<T> = (fn(&JointComponent) -> T, fn(&mut JointComponent, T));
/// `(id, min, max)`.
type IdRange = (u32, f32, f32);

/// Register the `Joint` namespace onto `lua`.
pub fn register<'lua, 'scope>(
    lua: &'lua Lua,
    scope: &mlua::Scope<'lua, 'scope>,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    let t = lua.create_table().map_err(|e| e.to_string())?;
    register_shape(scope, &t, scene)?;
    register_limits(scope, &t, scene)?;
    lua.globals().set("Joint", t).map_err(|e| e.to_string())
}

/// `id`'s Joint, cloned, or `None` without one.
fn get(scene: SceneCell, id: u32) -> Option<JointComponent> {
    scene.borrow().world.joint(id).map(|j| j.clone())
}

/// Apply `f` to `id`'s Joint, when it has one.
fn set(scene: SceneCell, id: u32, f: &dyn Fn(&mut JointComponent)) {
    if let Some(mut j) = scene.borrow_mut().world.joint_mut(id) {
        f(&mut j);
    }
}

/// `Get<name>` / `Set<name>` over one `f32` field (`0` without a Joint).
fn f32_pair<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    t: &Table<'lua>,
    scene: SceneCell<'scope>,
    name: &str,
    (read, write): Field<f32>,
) -> Reg {
    let f = scope.create_function(move |_, id: u32| Ok(get(scene, id).map_or(0.0, |j| read(&j))));
    put(t, &format!("Get{name}"), f)?;
    let f = scope.create_function(move |_, (id, v): (u32, f32)| {
        set(scene, id, &|j| write(j, v));
        Ok(())
    });
    put(t, &format!("Set{name}"), f)
}

/// `Get<name>` / `Set<name>` over one `bool` field (`false` without a Joint).
fn bool_pair<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    t: &Table<'lua>,
    scene: SceneCell<'scope>,
    name: &str,
    (read, write): Field<bool>,
) -> Reg {
    let f = scope.create_function(move |_, id: u32| Ok(get(scene, id).is_some_and(|j| read(&j))));
    put(t, &format!("Get{name}"), f)?;
    let f = scope.create_function(move |_, (id, v): (u32, bool)| {
        set(scene, id, &|j| write(j, v));
        Ok(())
    });
    put(t, &format!("Set{name}"), f)
}

/// `Get<name>` → `x, y, z` / `Set<name>(id, x, y, z)` over one `Vec3` field.
fn vec3_pair<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    t: &Table<'lua>,
    scene: SceneCell<'scope>,
    name: &str,
    (read, write): Field<Vec3>,
) -> Reg {
    let f = scope.create_function(move |_, id: u32| {
        let v = get(scene, id).map_or(Vec3::ZERO, |j| read(&j));
        Ok((v.x, v.y, v.z))
    });
    put(t, &format!("Get{name}"), f)?;
    let f = scope.create_function(move |_, (id, x, y, z): (u32, f32, f32, f32)| {
        set(scene, id, &|j| write(j, Vec3::new(x, y, z)));
        Ok(())
    });
    put(t, &format!("Set{name}"), f)
}

/// `Get/SetKind`, `Get/SetConnectedBody`, the anchors, the axis and
/// `Get/SetEnableCollision`.
fn register_shape<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    t: &Table<'lua>,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    let f = scope.create_function(move |_, id: u32| Ok(get(scene, id).map(|j| j.kind.name())));
    put(t, "GetKind", f)?;
    let f = scope.create_function(move |_, (id, name): (u32, String)| {
        if let Some(k) = JointKind::parse(&name) {
            set(scene, id, &|j| ops::set_kind(j, k));
        }
        Ok(())
    });
    put(t, "SetKind", f)?;
    let f =
        scope.create_function(move |_, id: u32| Ok(get(scene, id).and_then(|j| j.connected_body)));
    put(t, "GetConnectedBody", f)?;
    let f = scope.create_function(move |_, (id, body): (u32, Option<u32>)| {
        set(scene, id, &|j| ops::set_connected_body(j, body));
        Ok(())
    });
    put(t, "SetConnectedBody", f)?;
    vec3_pair(scope, t, scene, "Anchor", (|j| j.anchor, ops::set_anchor))?;
    let connected: Field<Vec3> = (|j| j.connected_anchor, ops::set_connected_anchor);
    vec3_pair(scope, t, scene, "ConnectedAnchor", connected)?;
    let auto: Field<bool> = (
        |j| j.auto_configure_connected_anchor,
        ops::set_auto_configure_connected_anchor,
    );
    bool_pair(scope, t, scene, "AutoConfigureConnectedAnchor", auto)?;
    vec3_pair(scope, t, scene, "Axis", (|j| j.axis, ops::set_axis))?;
    let collide: Field<bool> = (|j| j.enable_collision, ops::set_enable_collision);
    bool_pair(scope, t, scene, "EnableCollision", collide)
}

/// `Get/SetUseLimits`, `Get/SetLimits` (min, max degrees), `Get/SetSwingLimit`,
/// `Get/SetBreakForce`, `Get/SetBreakTorque`.
fn register_limits<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    t: &Table<'lua>,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    bool_pair(
        scope,
        t,
        scene,
        "UseLimits",
        (|j| j.use_limits, ops::set_use_limits),
    )?;
    let f = scope.create_function(move |_, id: u32| {
        let l = get(scene, id).map_or_else(Default::default, |j| j.limits);
        Ok((l.x, l.y))
    });
    put(t, "GetLimits", f)?;
    let f = scope.create_function(move |_, (id, min, max): IdRange| {
        set(scene, id, &|j| ops::set_limits(j, min, max));
        Ok(())
    });
    put(t, "SetLimits", f)?;
    let swing: Field<f32> = (|j| j.swing_limit, ops::set_swing_limit);
    f32_pair(scope, t, scene, "SwingLimit", swing)?;
    let force: Field<f32> = (|j| j.break_force, ops::set_break_force);
    f32_pair(scope, t, scene, "BreakForce", force)?;
    let torque: Field<f32> = (|j| j.break_torque, ops::set_break_torque);
    f32_pair(scope, t, scene, "BreakTorque", torque)
}
