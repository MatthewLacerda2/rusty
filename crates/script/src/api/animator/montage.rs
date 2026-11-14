//! crates/script/src/api/animator/montage.rs — the `Animator` montage surface (#1894).
//!
//! Unreal's `Montage_Play` family over the animator's slots: `PlayMontage`,
//! `StopMontage`, `IsMontagePlaying`, `GetMontage`, `JumpToSection` and
//! `SetNextSection`. A montage is resolved against the animator's graph asset
//! (read from disk, as `PlayNode` does), else as a clip name on the mesh, and
//! starts with a copy of what it plays (`components::ActiveMontage`), so the
//! `animate` system plays it without the graph. Unknown names, and a graph
//! that fails to load, warn and return `false`, never a Lua error (a graph
//! that exists but doesn't load never falls back to a clip of the same name);
//! a non-positive `rate` is one.

use std::path::Path;

use mlua::Table;

use super::{put, Reg};
use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use crate::scripting::{ConsoleLogs, LogLevel};
use rusty_asset::animation_graph;
use rusty_components::{ActiveMontage, MontageRefused};
use rusty_scene::Scene;

/// Register the montage functions onto the `Animator` table.
pub(super) fn register(
    lua: &mlua::Lua,
    table: &Table,
    scene: ApiCell<Scene>,
    console: ApiCell<ConsoleLogs>,
) -> Reg {
    register_play(lua, table, scene, console)?;
    register_stop(lua, table, scene)?;
    register_reads(lua, table, scene)?;
    register_sections(lua, table, scene)
}

/// `PlayMontage(id, montage [, { rate, start_section, slot, blend_in, blend_out }])`.
fn register_play(
    lua: &mlua::Lua,
    table: &Table,
    scene: ApiCell<Scene>,
    console: ApiCell<ConsoleLogs>,
) -> Reg {
    put(
        table,
        "PlayMontage",
        lua.create_function(
            move |lua, (id, name, opts): (EntityArg, String, Option<Table>)| {
                let id = id.live("Animator", "PlayMontage")?;
                let opts = Options::read(opts)?;
                let (scene, console) = (scene.get(lua)?, console.get(lua)?);
                let mut scene = scene.borrow_mut();
                let Some(path) = scene.world.animator(id).map(|a| a.graph.clone()) else {
                    return Ok(false);
                };
                let warn = |why: String| {
                    let msg = format!("Animator.PlayMontage: entity {id}: {why}");
                    console.borrow_mut().repeated(msg, LogLevel::Warning, 1);
                    Ok(false)
                };
                let graph = match path.map(|p| (animation_graph::load(Path::new(&p)), p)) {
                    None => None,
                    Some((Ok(graph), _)) => Some(graph),
                    Some((Err(err), p)) => return warn(format!("graph '{p}': {err}")),
                };
                let settings = (opts.slot.as_deref(), opts.start.as_deref(), opts.rate);
                let montage = {
                    let mesh = scene.world.mesh(id);
                    let clips = mesh.as_ref().map_or(&[][..], |m| &m.clips[..]);
                    ActiveMontage::resolve(graph.as_ref(), clips, &name, settings)
                };
                let montage = match montage {
                    Ok(m) => m.with_blends(opts.blend_in, opts.blend_out),
                    Err(why) => return warn(refused(&name, &why)),
                };
                let Some(mut anim) = scene.world.animator_mut(id) else {
                    return Ok(false);
                };
                anim.montages.play(montage);
                anim.is_playing = true;
                anim.freeze = false;
                Ok(true)
            },
        ),
    )
}

/// `PlayMontage`'s options table.
#[derive(Default)]
struct Options {
    rate: f32,
    start: Option<String>,
    slot: Option<String>,
    blend_in: Option<f32>,
    blend_out: Option<f32>,
}

impl Options {
    fn read(opts: Option<Table>) -> mlua::Result<Self> {
        let Some(t) = opts else {
            return Ok(Self {
                rate: 1.0,
                ..Self::default()
            });
        };
        let rate = t.get::<Option<f32>>("rate")?.unwrap_or(1.0);
        if !(rate.is_finite() && rate > 0.0) {
            return Err(mlua::Error::RuntimeError(format!(
                "Animator.PlayMontage: rate must be a positive number, got {rate}"
            )));
        }
        Ok(Self {
            rate,
            start: t.get("start_section")?,
            slot: t.get("slot")?,
            blend_in: t.get("blend_in")?,
            blend_out: t.get("blend_out")?,
        })
    }
}

/// Why `name` did not start, as the warning says it.
fn refused(name: &str, why: &MontageRefused) -> String {
    match why {
        MontageRefused::Unknown => format!("no montage or clip '{name}'"),
        MontageRefused::NoClip(clip) => format!("montage '{name}': the mesh has no clip '{clip}'"),
        MontageRefused::NoSlot(slot) => format!("montage '{name}': no slot '{slot}'"),
        MontageRefused::NoSection(s) => format!("montage '{name}': no section '{s}'"),
    }
}

/// `StopMontage(id [, slot [, blend_out]])`.
fn register_stop(lua: &mlua::Lua, table: &Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "StopMontage",
        lua.create_function(
            move |lua, (id, slot, blend): (EntityArg, Option<String>, Option<f32>)| {
                let id = id.live("Animator", "StopMontage")?;
                let scene = scene.get(lua)?;
                let mut scene = scene.borrow_mut();
                let Some(mut anim) = scene.world.animator_mut(id) else {
                    return Ok(false);
                };
                let blend = blend.filter(|b| b.is_finite()).map(|b| b.max(0.0));
                Ok(anim.montages.stop(slot.as_deref(), blend))
            },
        ),
    )
}

/// `IsMontagePlaying(id [, slot])` and `GetMontage(id, slot)`.
fn register_reads(lua: &mlua::Lua, table: &Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "IsMontagePlaying",
        lua.create_function(move |lua, (id, slot): (EntityArg, Option<String>)| {
            let scene = scene.get(lua)?;
            let scene = scene.borrow();
            let anim = scene.world.animator(id.read());
            Ok(anim.is_some_and(|a| a.montages.active(slot.as_deref()).is_some()))
        }),
    )?;
    put(
        table,
        "GetMontage",
        lua.create_function(move |lua, (id, slot): (EntityArg, String)| {
            let scene = scene.get(lua)?;
            let scene = scene.borrow();
            let Some(anim) = scene.world.animator(id.read()) else {
                return Ok((None, None));
            };
            Ok(match anim.montages.active(Some(&slot)) {
                Some(m) => (
                    Some(m.montage.name.clone()),
                    Some(m.section_name().to_string()),
                ),
                None => (None, None),
            })
        }),
    )
}

/// `JumpToSection(id, section [, slot])` and `SetNextSection(id, from, to [, slot])`.
fn register_sections(lua: &mlua::Lua, table: &Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "JumpToSection",
        lua.create_function(
            move |lua, (id, section, slot): (EntityArg, String, Option<String>)| {
                let id = id.live("Animator", "JumpToSection")?;
                let scene = scene.get(lua)?;
                let mut scene = scene.borrow_mut();
                let Some(mut anim) = scene.world.animator_mut(id) else {
                    return Ok(false);
                };
                Ok(anim.montages.jump(&section, slot.as_deref()))
            },
        ),
    )?;
    put(
        table,
        "SetNextSection",
        lua.create_function(
            move |lua, (id, from, to, slot): (EntityArg, String, String, Option<String>)| {
                let id = id.live("Animator", "SetNextSection")?;
                let scene = scene.get(lua)?;
                let mut scene = scene.borrow_mut();
                let Some(mut anim) = scene.world.animator_mut(id) else {
                    return Ok(false);
                };
                Ok(anim.montages.set_next(&from, &to, slot.as_deref()))
            },
        ),
    )
}
