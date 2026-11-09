//! src/api/physics/collider.rs — `Physics.GetColliderShape` / `SetColliderShape` /
//! `GetPhysicsMaterial` / `SetPhysicsMaterial` (#447).
//!
//! The collider's authorable data — its shape and its physics material — over the
//! same `scene::authoring::physics::collider` ops the inspector's Collider card writes
//! through. Unity: the `*Collider` component's size fields and its
//! `sharedMaterial` (`PhysicMaterial`).

use glam::Vec3;
use mlua::{Lua, Table};

use super::super::{put, Reg};
use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use crate::components::{CapsuleAxis, ColliderShape, CombineMode, PhysicsMaterial};
use crate::scene::asset_instance::collision;
use crate::scene::authoring::physics::collider as collider_ops;
use crate::scene::Scene;

fn err(msg: String) -> mlua::Error {
    mlua::Error::RuntimeError(msg)
}

/// Register the collider shape + material accessors onto the `Physics` table.
pub(super) fn register(lua: &mlua::Lua, table: &Table, scene: ApiCell<Scene>) -> Reg {
    register_shape(lua, table, scene)?;
    register_material(lua, table, scene)
}

/// `GetColliderShape(id)` → a `{ kind = .., .. }` table (or `nil` without a
/// collider); `SetColliderShape(id, shape)` takes the same table back.
fn register_shape(lua: &mlua::Lua, table: &Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "GetColliderShape",
        lua.create_function(move |lua, id: EntityArg| {
            let scene = &*scene.get(lua)?;
            let shape = scene.borrow().world.collider(id.read()).map(|c| {
                let mesh = c.shared_mesh.as_ref().and_then(|m| m.asset_ref.clone());
                (c.shape.clone(), mesh)
            });
            let Some((shape, mesh)) = shape else {
                return Ok(None);
            };
            let t = shape_to_table(lua, &shape)?;
            // The collision mesh a Mesh shape builds from, when it is not the Mesh's.
            if matches!(shape, ColliderShape::Mesh { .. }) {
                t.set("mesh", mesh)?;
            }
            Ok(Some(t))
        }),
    )?;
    put(
        table,
        "SetColliderShape",
        lua.create_function(move |lua, (id, t): (EntityArg, Table)| {
            let scene = &*scene.get(lua)?;
            let id = id.live("Physics", "SetColliderShape")?;
            if t.get::<String>("kind")? == "Mesh" {
                return set_mesh_shape(&mut scene.borrow_mut(), id, &t);
            }
            let shape = shape_from_table(&t)?;
            let mut scene = scene.borrow_mut();
            if let Some(mut c) = scene.world.collider_mut(id) {
                collider_ops::set_shape(&mut c, shape);
            }
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
            t.set("axis", axis.name())?;
        }
        ColliderShape::Mesh { convex, .. } => {
            t.set("kind", "Mesh")?;
            t.set("convex", *convex)?;
        }
    }
    Ok(t)
}

/// `SetColliderShape(id, { kind = "Mesh", convex = false, mesh = nil })` (#1960): a
/// mesh collider built from the collision mesh `mesh` names (`path::sub_object`, or
/// a sub-object of the entity's own model), else from the entity's Mesh. A name the
/// model lacks warns once and falls back; nothing to build from is an error. An
/// entity without a collider is left alone, as for every other kind.
fn set_mesh_shape(scene: &mut Scene, id: u32, t: &Table) -> mlua::Result<()> {
    if !scene.world.has_collider(id) {
        return Ok(());
    }
    let convex = t.get::<Option<bool>>("convex")?.unwrap_or(false);
    let mesh = t.get::<Option<String>>("mesh")?;
    collision::set_mesh_shape(scene, id, convex, mesh.as_deref())
        .map_err(|e| err(format!("SetColliderShape: {e}")))
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
                        "SetColliderShape: unknown axis {a:?} (want one of {})",
                        CapsuleAxis::names()
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
                "SetColliderShape: unknown kind {kind:?} (want Box, Sphere, Cylinder, \
                 Capsule or Mesh)"
            )))
        }
    })
}

/// `GetPhysicsMaterial(id)` → `friction, bounciness, friction_combine,
/// bounce_combine`: the values the solver uses, its asset's when it names one
/// (#1878), the defaults without a collider;
/// `SetPhysicsMaterial(id, friction, bounciness [, friction_combine [, bounce_combine]])`
/// gives the collider its own inline values, dropping a shared asset (Unity's
/// `Collider.material`, which instances the shared one).
fn register_material(lua: &mlua::Lua, table: &Table, scene: ApiCell<Scene>) -> Reg {
    type MaterialArgs = (EntityArg, f32, f32, Option<String>, Option<String>);
    put(
        table,
        "GetPhysicsMaterial",
        lua.create_function(move |lua, id: EntityArg| {
            let scene = &*scene.get(lua)?;
            let m = scene
                .borrow()
                .world
                .collider(id.read())
                .map(|c| crate::scene::physics_material::effective(&c))
                .unwrap_or_default();
            Ok((
                m.friction,
                m.bounciness,
                m.friction_combine.name(),
                m.bounce_combine.name(),
            ))
        }),
    )?;
    put(
        table,
        "SetPhysicsMaterial",
        lua.create_function(
            move |lua, (id, friction, bounciness, fc, bc): MaterialArgs| {
                let scene = &*scene.get(lua)?;
                let id = id.live("Physics", "SetPhysicsMaterial")?;
                if !friction.is_finite() || !bounciness.is_finite() {
                    return Err(err(
                        "SetPhysicsMaterial: friction and bounciness must be finite".into(),
                    ));
                }
                let material = PhysicsMaterial {
                    friction_combine: combine_mode(fc, "SetPhysicsMaterial")?,
                    bounce_combine: combine_mode(bc, "SetPhysicsMaterial")?,
                    ..PhysicsMaterial::new(friction, bounciness)
                };
                let mut scene = scene.borrow_mut();
                if let Some(mut c) = scene.world.collider_mut(id) {
                    collider_ops::set_material(&mut c, material);
                    c.material_asset = None; // its own values now, not the asset's
                }
                Ok(())
            },
        ),
    )
}

/// An optional combine-mode name; absent means `Average`. `verb` names the
/// caller in the error.
pub(super) fn combine_mode(name: Option<String>, verb: &str) -> mlua::Result<CombineMode> {
    let Some(name) = name else {
        return Ok(CombineMode::default());
    };
    CombineMode::parse(&name).ok_or_else(|| {
        err(format!(
            "{verb}: unknown combine mode {name:?} (want one of {})",
            CombineMode::names()
        ))
    })
}
