//! src/api/animator/ — `Animator` namespace.
//!
//! `Play`/`Crossfade`/`Stop` plus `Pause`/`Resume` and `SetLooping` (#313) over an
//! entity's optional animator component. `Play` hard-cuts to a clip; `Crossfade`
//! blends out of the current clip over a duration (#80); both drive the component's
//! minimal state machine, which the `animate` system samples against the mesh's
//! imported clips. `Pause`/`Resume` hold and release the playhead over the
//! component's `freeze` flag; `SetLooping` wraps the playhead at the clip's end.
//! `SetBool`/`SetFloat`/`SetInt`/`SetTrigger` (#314) write the typed graph
//! parameters the animation-graph evaluator (#316) reads to decide transitions.
//! `GetBone` (#453) finds a bone GameObject of the entity's skeleton by name, and
//! `GetClip` (#1384) reads the base layer's current clip.
//! The graph control surface (#316: `SetGraph`, `SetGraphEnabled`, `PlayNode`,
//! `PlayAnimation`) lives in `graph.rs`, and the layer surface (#457:
//! `SetLayerWeight`, `GetLayerWeight`, `GetCurrentNode`) in `layers.rs`, and the
//! IK surface (#461: `Add*IK`, `RemoveIK`, `SetIK*`, `GetIKWeight`) in `ik.rs`,
//! the playback-speed surface (#1342: `SetSpeed`, `GetSpeed`) in `speed.rs`, and the
//! typed parameters (`SetBool`/`SetFloat`/`SetInt`/`SetTrigger`) in `parameters.rs` —
//! one namespace, split to stay under the size cap.

mod graph;
mod ik;
mod layers;
mod parameters;
mod speed;

use mlua::Lua;

use super::{put, Reg};
use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use crate::scene::authoring::animator as animator_ops;
use crate::scene::Scene;
use crate::scripting::ConsoleLogs;

/// Register the `Animator` namespace onto `lua`.
pub fn register(lua: &Lua, scene: ApiCell<Scene>, console: ApiCell<ConsoleLogs>) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;

    register_play(lua, &table, scene, console)?;
    register_stop(lua, &table, scene)?;
    register_pause_resume(lua, &table, scene)?;
    register_set_looping(lua, &table, scene)?;
    speed::register(lua, &table, scene)?;
    parameters::register(lua, &table, scene, console)?;
    register_get_bone(lua, &table, scene)?;
    graph::register(lua, &table, scene, console)?;
    layers::register(lua, &table, scene, console)?;
    ik::register(lua, &table, scene, console)?;

    lua.globals()
        .set("Animator", table)
        .map_err(|e| e.to_string())
}

/// `Play` / `Crossfade` — start a clip (crossfade simplifies to a plain play).
fn register_play(
    lua: &mlua::Lua,
    table: &mlua::Table,
    scene: ApiCell<Scene>,
    console: ApiCell<ConsoleLogs>,
) -> Reg {
    put(
        table,
        "Play",
        lua.create_function(move |lua, (id, clip): (EntityArg, String)| {
            scene.with(lua, |scene| {
                let id = id.live("Animator", "Play")?;
                let mut scene = scene.borrow_mut();
                let played = scene
                    .world
                    .animator_mut(id)
                    .map(|mut anim| anim.play(clip.clone()))
                    .is_some();
                if played {
                    let name = scene.world.name(id).map(|n| n.clone()).unwrap_or_default();
                    console
                        .get(lua)?
                        .borrow_mut()
                        .info(format!("Entity {} playing animation: {}", name, clip));
                }
                Ok(())
            })
        }),
    )?;

    put(
        table,
        "Crossfade",
        lua.create_function(move |lua, (id, clip, duration): (EntityArg, String, f32)| {
            let scene = &*scene.get(lua)?;
            let id = id.live("Animator", "Crossfade")?;
            // Blend out of the current clip over `duration` seconds (#80).
            let mut scene = scene.borrow_mut();
            if let Some(mut anim) = scene.world.animator_mut(id) {
                anim.crossfade(clip, duration);
            }
            Ok(())
        }),
    )
}

/// `GetBone` — the bone GameObject of the entity's skeleton named `name` (#453),
/// or `nil`. Bones are ordinary entities: parent to one, move it, add to it. Also
/// `GetClip`, the other per-entity read.
fn register_get_bone(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "GetBone",
        lua.create_function(move |lua, (id, name): (EntityArg, String)| {
            Ok(scene.get(lua)?.borrow().find_bone(id.read(), &name))
        }),
    )?;
    // `GetClip` (#1384): the base layer's current clip, Unity's
    // `GetCurrentAnimatorClipInfo(0)`; `nil` without an animator.
    put(
        table,
        "GetClip",
        lua.create_function(move |lua, id: EntityArg| {
            let scene = &*scene.get(lua)?;
            let scene = scene.borrow();
            Ok(scene
                .world
                .animator(id.read())
                .map(|a| a.base.current_clip.clone()))
        }),
    )
}

/// `Stop` — halt playback on the entity's animator.
fn register_stop(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "Stop",
        lua.create_function(move |lua, id: EntityArg| {
            let scene = &*scene.get(lua)?;
            let id = id.live("Animator", "Stop")?;
            let mut scene = scene.borrow_mut();
            if let Some(mut c) = scene.world.animator_mut(id) {
                animator_ops::set_playing(&mut c, false);
            }
            Ok(())
        }),
    )
}

/// `Pause` / `Resume` — hold and release the playhead over the component's `freeze`
/// flag (#313). Unlike `Stop`, `is_playing` stays set: the pose holds where it is
/// and `Resume` continues from the same frame.
fn register_pause_resume(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "Pause",
        lua.create_function(move |lua, id: EntityArg| {
            let scene = &*scene.get(lua)?;
            let id = id.live("Animator", "Pause")?;
            let mut scene = scene.borrow_mut();
            if let Some(mut c) = scene.world.animator_mut(id) {
                animator_ops::set_freeze(&mut c, true);
            }
            Ok(())
        }),
    )?;

    put(
        table,
        "Resume",
        lua.create_function(move |lua, id: EntityArg| {
            let scene = &*scene.get(lua)?;
            let id = id.live("Animator", "Resume")?;
            let mut scene = scene.borrow_mut();
            if let Some(mut c) = scene.world.animator_mut(id) {
                animator_ops::set_freeze(&mut c, false);
            }
            Ok(())
        }),
    )
}

/// `SetLooping` — wrap the playhead at the current clip's end so it repeats
/// seamlessly (idle/run/walk cycles, #313). Off, the last frame holds as before.
fn register_set_looping(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "SetLooping",
        lua.create_function(move |lua, (id, looping): (EntityArg, bool)| {
            let scene = &*scene.get(lua)?;
            let id = id.live("Animator", "SetLooping")?;
            let mut scene = scene.borrow_mut();
            if let Some(mut c) = scene.world.animator_mut(id) {
                animator_ops::set_looping(&mut c, looping);
            }
            Ok(())
        }),
    )
}
