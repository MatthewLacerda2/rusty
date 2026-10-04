//! src/api/physics/collider.rs — `Physics.GetColliderShape` / `SetColliderShape` /
//! `GetPhysicsMaterial` / `SetPhysicsMaterial` (#447).
//!
//! The collider's authorable data — its shape and its physics material — over the
//! same `scene::authoring::collider` ops the inspector's Collider card writes
//! through. Unity: the `*Collider` component's size fields and its
//! `sharedMaterial` (`PhysicMaterial`).

use std::cell::RefCell;

use glam::Vec3;
use mlua::{Lua, Table};

use super::super::{put, Reg};
use crate::components::{CapsuleAxis, ColliderShape, CombineMode, PhysicsMaterial};
use crate::scene::authoring::collider as collider_ops;
use crate::scene::Scene;

fn err(msg: String) -> mlua::Error {
    mlua::Error::RuntimeError(msg)
}

/// Register the collider shape + material accessors onto the `Physics` table.
pub(super) fn register<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    register_shape(scope, table, scene)?;
    register_material(scope, table, scene)
}

/// `GetColliderShape(id)` → a `{ kind = .., .. }` table (or `nil` without a
/// collider); `SetColliderShape(id, shape)` takes the same table back.
fn register_shape<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    put(
        table,
        "GetColliderShape",
        scope.create_function(|lua, id: u32| {
            let shape = scene.borrow().world.collider(id).map(|c| c.shape.clone());
            shape.map(|s| shape_to_table(lua, &s)).transpose()
        }),
    )?;
    put(
        table,
        "SetColliderShape",
        scope.create_function(|_, (id, t): (u32, Table)| {
            let shape = shape_from_table(&t)?;
            let mut scene = scene.borrow_mut();
            if let Some(mut c) = scene.world.collider_mut(id) {
                collider_ops::set_shape(&mut c, shape);
            }
            scene.update_entity_collider(id);
            Ok(())
        }),
    )
}

fn shape_to_table(lua: &Lua, shape: &ColliderShape) -> mlua::Result<Table> {
    let t = lua.create_table()?;
    match shape {
        ColliderShape::Box { size } => {
            t.set("kind", "Box")?;
            t.set("x", size.x)?;
            t.set("y", size.y)?;
            t.set("z", size.z)?;
        }
        ColliderShape::Sphere { radius } => {
            t.set("kind", "Sphere")?;
            t.set("radius", *radius)?;
        }
        ColliderShape::Cylinder { radius, height } => {
            t.set("kind", "Cylinder")?;
            t.set("radius", *radius)?;
            t.set("height", *height)?;
        }
        ColliderShape::Capsule {
            radius,
            height,
            axis,
        } => {
            t.set("kind", "Capsule")?;
            t.set("radius", *radius)?;
            t.set("height", *height)?;
            t.set("axis", axis.as_str())?;
        }
        ColliderShape::Mesh { convex, .. } => {
            t.set("kind", "Mesh")?;
            t.set("convex", *convex)?;
        }
    }
    Ok(t)
}

/// A strictly positive, finite extent named `key` of the shape table.
fn extent(t: &Table, key: &str) -> mlua::Result<f32> {
    let v: f32 = t
        .get::<Option<f32>>(key)?
        .ok_or_else(|| err(format!("SetColliderShape: missing {key:?}")))?;
    if v.is_finite() && v > 0.0 {
        Ok(v)
    } else {
        Err(err(format!(
            "SetColliderShape: {key:?} must be > 0, got {v}"
        )))
    }
}

fn shape_from_table(t: &Table) -> mlua::Result<ColliderShape> {
    let kind: String = t.get("kind")?;
    Ok(match kind.as_str() {
        "Box" => ColliderShape::Box {
            size: Vec3::new(extent(t, "x")?, extent(t, "y")?, extent(t, "z")?),
        },
        "Sphere" => ColliderShape::Sphere {
            radius: extent(t, "radius")?,
        },
        "Cylinder" => ColliderShape::Cylinder {
            radius: extent(t, "radius")?,
            height: extent(t, "height")?,
        },
        "Capsule" => {
            let axis = match t.get::<Option<String>>("axis")? {
                None => CapsuleAxis::default(),
                Some(a) => CapsuleAxis::parse(&a).ok_or_else(|| {
                    err(format!(
                        "SetColliderShape: unknown axis {a:?} (want \"X\", \"Y\" or \"Z\")"
                    ))
                })?,
            };
            ColliderShape::Capsule {
                radius: extent(t, "radius")?,
                height: extent(t, "height")?,
                axis,
            }
        }
        _ => {
            return Err(err(format!(
                "SetColliderShape: unknown kind {kind:?} (want Box, Sphere, Cylinder or \
                 Capsule; a Mesh collider is baked from an imported mesh)"
            )))
        }
    })
}

/// `GetPhysicsMaterial(id)` → `friction, bounciness, friction_combine,
/// bounce_combine` (the defaults without a collider);
/// `SetPhysicsMaterial(id, friction, bounciness [, friction_combine [, bounce_combine]])`.
fn register_material<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    put(
        table,
        "GetPhysicsMaterial",
        scope.create_function(|_, id: u32| {
            let m = scene
                .borrow()
                .world
                .collider(id)
                .map(|c| c.material)
                .unwrap_or_default();
            Ok((
                m.friction,
                m.bounciness,
                m.friction_combine.as_str(),
                m.bounce_combine.as_str(),
            ))
        }),
    )?;
    put(
        table,
        "SetPhysicsMaterial",
        scope.create_function(
            |_,
             (id, friction, bounciness, fc, bc): (
                u32,
                f32,
                f32,
                Option<String>,
                Option<String>,
            )| {
                if !friction.is_finite() || !bounciness.is_finite() {
                    return Err(err(
                        "SetPhysicsMaterial: friction and bounciness must be finite".into(),
                    ));
                }
                let material = PhysicsMaterial {
                    friction_combine: combine_mode(fc)?,
                    bounce_combine: combine_mode(bc)?,
                    ..PhysicsMaterial::new(friction, bounciness)
                };
                let mut scene = scene.borrow_mut();
                if let Some(mut c) = scene.world.collider_mut(id) {
                    collider_ops::set_material(&mut c, material);
                }
                Ok(())
            },
        ),
    )
}

/// An optional combine-mode name; absent means `Average`.
fn combine_mode(name: Option<String>) -> mlua::Result<CombineMode> {
    let Some(name) = name else {
        return Ok(CombineMode::default());
    };
    CombineMode::parse(&name).ok_or_else(|| {
        err(format!(
            "SetPhysicsMaterial: unknown combine mode {name:?} (want Average, Minimum, \
             Multiply or Maximum)"
        ))
    })
}
