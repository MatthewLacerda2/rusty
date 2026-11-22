use glam::Vec3;
use mlua::{Lua, Table};

use super::super::{put, Reg};
use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use rusty_components::{CapsuleAxis, ColliderShape, CombineMode, PhysicsMaterial};
use rusty_edit::asset_instance::collision;
use rusty_edit::authoring::physics::collider as collider_ops;
use rusty_scene::Scene;

fn err(msg: String) -> mlua::Error {
    mlua::Error::RuntimeError(msg)
}

pub(super) fn register(lua: &mlua::Lua, table: &Table, scene: ApiCell<Scene>) -> Reg {
    register_shape(lua, table, scene)?;
    register_material(lua, table, scene)
}

fn register_shape(lua: &mlua::Lua, table: &Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "GetColliderShape",
        lua.create_function(move |lua, id: EntityArg| {
            let scene = &*scene.get(lua)?;
            let shape = scene.borrow().world.collider(id.read()).map(|c| {
                let mesh = c.collision_mesh.as_ref().and_then(|m| m.asset_ref.clone());
                (c.shape.clone(), mesh)
            });
            let Some((shape, mesh)) = shape else {
                return Ok(None);
            };
            let t = shape_to_table(lua, &shape)?;
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

fn set_mesh_shape(scene: &mut Scene, id: u32, t: &Table) -> mlua::Result<()> {
    let convex = t.get::<Option<bool>>("convex")?.unwrap_or(false);
    let mesh = t.get::<Option<String>>("mesh")?;
    collision::set_mesh_shape(scene, id, convex, mesh.as_deref())
        .map_err(|e| err(format!("SetColliderShape: {e}")))
}

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
                .map(|c| rusty_scene::physics_material::effective(&c))
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
                    c.material_asset = None;
                }
                Ok(())
            },
        ),
    )
}

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
