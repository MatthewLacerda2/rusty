//! crates/script/src/api/animator/ik.rs — the `Animator` IK surface (#461).
//!
//! `AddTwoBoneIK` / `AddAimIK` / `RemoveIK` author an animator's named IK
//! constraints (saved with it); `SetIKTarget[Entity]`, `SetIKHint[Entity]` and
//! `SetIKWeight` / `GetIKWeight` drive them at runtime — Unity's
//! `Animator.SetIKPosition` / `SetIKHintPosition` / `SetIKPositionWeight` and
//! `SetLookAtPosition`, generalised to named chains over bone GameObjects. The
//! solve itself is `Scene::solve_ik`, after `LateUpdate`.

use std::cell::RefCell;

use glam::Vec3;
use mlua::Table;

use super::{put, Reg};
use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use crate::scripting::ConsoleLogs;
use rusty_components::{IkChain, IkConstraint, IkTarget};
use rusty_scene::Scene;

/// Register the IK functions onto the `Animator` table.
pub(super) fn register(
    lua: &mlua::Lua,
    table: &Table,
    scene: ApiCell<Scene>,
    console: ApiCell<ConsoleLogs>,
) -> Reg {
    register_add(lua, table, scene, console)?;
    register_remove(lua, table, scene)?;
    register_targets(lua, table, scene, console)?;
    register_weight(lua, table, scene, console)
}

/// Add (or replace) `constraint` on `id`'s animator once every bone it names
/// exists in `id`'s skeleton; otherwise warn and refuse.
fn add(scene: &RefCell<Scene>, console: &RefCell<ConsoleLogs>, id: u32, c: IkConstraint) -> bool {
    let mut scene = scene.borrow_mut();
    let missing = c
        .bone_names()
        .into_iter()
        .find(|n| scene.find_bone(id, n).is_none());
    let problem = match (scene.world.has_animator(id), missing, c.name.is_empty()) {
        (false, ..) => format!("entity {id} has no Animator"),
        (_, _, true) => "the constraint needs a name".to_string(),
        (_, Some(bone), _) => format!("entity {id} has no bone '{bone}'"),
        _ => {
            if let Some(mut anim) = scene.world.animator_mut(id) {
                anim.set_ik(c);
            }
            return true;
        }
    };
    console
        .borrow_mut()
        .warn(format!("Animator.Add*IK: {problem}"));
    false
}

/// `AddTwoBoneIK(id, name, root, mid, tip)` and
/// `AddAimIK(id, name, bones [, { weights, axis, clamp, weight }])`.
fn register_add(
    lua: &mlua::Lua,
    table: &Table,
    scene: ApiCell<Scene>,
    console: ApiCell<ConsoleLogs>,
) -> Reg {
    put(
        table,
        "AddTwoBoneIK",
        lua.create_function(
            move |lua, (id, name, root, mid, tip): (EntityArg, String, String, String, String)| {
                let id = id.live("Animator", "AddTwoBoneIK")?;
                let chain = IkChain::TwoBone { root, mid, tip };
                Ok(add(
                    &*scene.get(lua)?,
                    &*console.get(lua)?,
                    id,
                    IkConstraint::new(name, chain),
                ))
            },
        ),
    )?;
    put(
        table,
        "AddAimIK",
        lua.create_function(
            move |lua, (id, name, bones, opts): (EntityArg, String, Vec<String>, Option<Table>)| {
                let console = &*console.get(lua)?;
                let id = id.live("Animator", "AddAimIK")?;
                if bones.is_empty() {
                    console
                        .borrow_mut()
                        .warn("Animator.AddAimIK: the chain needs at least one bone".into());
                    return Ok(false);
                }
                let mut c = IkConstraint::aim(name, bones);
                if let Some(opts) = opts {
                    aim_options(&mut c, &opts)?;
                }
                Ok(add(&*scene.get(lua)?, console, id, c))
            },
        ),
    )
}

/// Read `{ weights = {..}, axis = {x, y, z}, clamp = degrees, weight = w }`.
fn aim_options(c: &mut IkConstraint, t: &Table) -> mlua::Result<()> {
    if let Some(w) = t.get::<Option<f32>>("weight")? {
        c.weight = w.clamp(0.0, 1.0);
    }
    let IkChain::Aim {
        weights,
        axis,
        clamp_degrees,
        ..
    } = &mut c.chain
    else {
        return Ok(());
    };
    if let Some(w) = t.get::<Option<Vec<f32>>>("weights")? {
        *weights = w;
    }
    if let Some([x, y, z]) = t.get::<Option<[f32; 3]>>("axis")? {
        *axis = Vec3::new(x, y, z);
    }
    if let Some(degrees) = t.get::<Option<f32>>("clamp")? {
        *clamp_degrees = degrees.max(0.0);
    }
    Ok(())
}

/// `RemoveIK(id, name)` — drop the constraint, putting back what it last wrote.
fn register_remove(lua: &mlua::Lua, table: &Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "RemoveIK",
        lua.create_function(move |lua, (id, name): (EntityArg, String)| {
            let scene = &*scene.get(lua)?;
            let id = id.live("Animator", "RemoveIK")?;
            let mut scene = scene.borrow_mut();
            let removed = scene.world.animator_mut(id).and_then(|mut a| {
                let at = a.ik.iter().position(|c| c.name == name)?;
                Some(a.ik.remove(at))
            });
            Ok(removed.map(|mut c| scene.undo_ik(&mut c)).is_some())
        }),
    )
}

/// Run `f` on `id`'s constraint `name`; warn and return `false` when absent.
fn with_ik(
    scene: &RefCell<Scene>,
    console: &RefCell<ConsoleLogs>,
    (func, id, name): (&str, u32, &str),
    f: impl FnOnce(&mut IkConstraint),
) -> bool {
    let mut scene = scene.borrow_mut();
    let found = scene
        .world
        .animator_mut(id)
        .and_then(|mut a| a.ik_mut(name).map(f));
    if found.is_none() {
        console
            .borrow_mut()
            .warn(format!("Animator.{func}: entity {id} has no IK '{name}'"));
    }
    found.is_some()
}

/// `SetIKTarget(id, name, x, y, z)`, `SetIKTargetEntity(id, name, target)`,
/// `SetIKHint(id, name, x, y, z)`, `SetIKHintEntity(id, name, hint)`.
fn register_targets(
    lua: &mlua::Lua,
    table: &Table,
    scene: ApiCell<Scene>,
    console: ApiCell<ConsoleLogs>,
) -> Reg {
    for (func, hint) in [("SetIKTarget", false), ("SetIKHint", true)] {
        let set = move |c: &mut IkConstraint, t: IkTarget| {
            let slot = if hint { &mut c.hint } else { &mut c.target };
            *slot = Some(t);
        };
        put(
            table,
            func,
            lua.create_function(
                move |lua, (id, name, x, y, z): (EntityArg, String, f32, f32, f32)| {
                    let id = id.live("Animator", func)?;
                    let point = IkTarget::Point(Vec3::new(x, y, z));
                    Ok(with_ik(
                        &*scene.get(lua)?,
                        &*console.get(lua)?,
                        (func, id, &name),
                        |c| set(c, point),
                    ))
                },
            ),
        )?;
        let entity_func = if hint {
            "SetIKHintEntity"
        } else {
            "SetIKTargetEntity"
        };
        put(
            table,
            entity_func,
            lua.create_function(
                move |lua, (id, name, target): (EntityArg, String, EntityArg)| {
                    let id = id.live("Animator", entity_func)?;
                    let target = target.live("Animator", entity_func)?;
                    let to = IkTarget::Entity(target);
                    Ok(with_ik(
                        &*scene.get(lua)?,
                        &*console.get(lua)?,
                        (entity_func, id, &name),
                        |c| set(c, to),
                    ))
                },
            ),
        )?;
    }
    Ok(())
}

/// `SetIKWeight(id, name, w)` (clamped to `[0, 1]`) and `GetIKWeight(id, name)`.
fn register_weight(
    lua: &mlua::Lua,
    table: &Table,
    scene: ApiCell<Scene>,
    console: ApiCell<ConsoleLogs>,
) -> Reg {
    put(
        table,
        "SetIKWeight",
        lua.create_function(move |lua, (id, name, w): (EntityArg, String, f32)| {
            let id = id.live("Animator", "SetIKWeight")?;
            let w = w.clamp(0.0, 1.0);
            Ok(with_ik(
                &*scene.get(lua)?,
                &*console.get(lua)?,
                ("SetIKWeight", id, &name),
                |c| c.weight = w,
            ))
        }),
    )?;
    put(
        table,
        "GetIKWeight",
        lua.create_function(move |lua, (id, name): (EntityArg, String)| {
            let scene = &*scene.get(lua)?;
            let mut scene = scene.borrow_mut();
            let mut anim = scene.world.animator_mut(id.read());
            Ok(anim
                .as_mut()
                .and_then(|a| a.ik_mut(&name).map(|c| c.weight)))
        }),
    )
}
