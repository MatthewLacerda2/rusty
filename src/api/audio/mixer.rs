//! src/api/audio/mixer.rs — the `Audio` mixer verbs (#465).
//!
//! Groups (buses), snapshots and ducking on the `AudioMaestro`'s mixer: scripts
//! create groups, set their volume / mute / filters / reverb send, define snapshots
//! and blend to them over sim time, add duck rules, and read any group's live state
//! back. Unknown group or snapshot names are errors naming what does exist, so a
//! typo never fails silently.

use std::cell::RefCell;

use mlua::{Lua, Table};

use super::super::{put, Reg};
use crate::audio::mixer::{Filter, GroupPatch, GroupState};
use crate::audio::AudioMaestro;
use crate::time::Time;

/// A duck's default attack and release, in seconds.
const DUCK_ATTACK: f32 = 0.1;
const DUCK_RELEASE: f32 = 0.5;

/// The fields a snapshot entry may name.
const PATCH_FIELDS: [&str; 7] = [
    "volume",
    "mute",
    "low_pass",
    "low_pass_resonance",
    "high_pass",
    "high_pass_resonance",
    "reverb_send",
];

fn err(e: String) -> mlua::Error {
    mlua::Error::RuntimeError(format!("Audio: {e}"))
}

/// Register the mixer verbs onto the `Audio` table.
pub fn register<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &Table<'lua>,
    audio: &'scope RefCell<AudioMaestro>,
    time: &'scope RefCell<Time>,
) -> Reg {
    register_groups(scope, table, audio)?;
    register_snapshots(scope, table, audio, time)
}

/// Apply `patch` to group `name`.
fn set(audio: &RefCell<AudioMaestro>, name: &str, patch: GroupPatch) -> mlua::Result<()> {
    audio.borrow_mut().set_group(name, &patch).map_err(err)
}

fn register_groups<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &Table<'lua>,
    audio: &'scope RefCell<AudioMaestro>,
) -> Reg {
    put(
        table,
        "GetGroups",
        scope.create_function(|_, ()| {
            let mixer = audio.borrow();
            Ok(mixer
                .mixer()
                .groups()
                .iter()
                .map(|g| g.name.clone())
                .collect::<Vec<_>>())
        }),
    )?;
    put(
        table,
        "CreateGroup",
        scope.create_function(|_, (name, parent): (String, Option<String>)| {
            audio
                .borrow_mut()
                .create_group(&name, parent.as_deref())
                .map_err(err)
        }),
    )?;
    put(
        table,
        "SetGroupVolume",
        scope.create_function(|_, (name, v): (String, f32)| {
            set(
                audio,
                &name,
                GroupPatch {
                    volume: Some(v),
                    ..Default::default()
                },
            )
        }),
    )?;
    put(
        table,
        "SetGroupMute",
        scope.create_function(|_, (name, mute): (String, bool)| {
            set(
                audio,
                &name,
                GroupPatch {
                    mute: Some(mute),
                    ..Default::default()
                },
            )
        }),
    )?;
    put(
        table,
        "SetGroupLowPass",
        scope.create_function(|_, (name, hz, q): (String, f32, Option<f32>)| {
            let low_pass = Some(Filter::new(hz, q.unwrap_or(0.0)));
            set(
                audio,
                &name,
                GroupPatch {
                    low_pass,
                    ..Default::default()
                },
            )
        }),
    )?;
    put(
        table,
        "SetGroupHighPass",
        scope.create_function(|_, (name, hz, q): (String, f32, Option<f32>)| {
            let high_pass = Some(Filter::new(hz, q.unwrap_or(0.0)));
            set(
                audio,
                &name,
                GroupPatch {
                    high_pass,
                    ..Default::default()
                },
            )
        }),
    )?;
    put(
        table,
        "SetGroupReverbSend",
        scope.create_function(|_, (name, v): (String, f32)| {
            set(
                audio,
                &name,
                GroupPatch {
                    reverb_send: Some(v),
                    ..Default::default()
                },
            )
        }),
    )?;
    put(
        table,
        "GetGroupState",
        scope.create_function(|lua, name: String| {
            let maestro = audio.borrow();
            let id = maestro.mixer().require(&name).map_err(err)?;
            state_table(lua, &maestro.mixer().state(id))
        }),
    )
}

fn register_snapshots<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &Table<'lua>,
    audio: &'scope RefCell<AudioMaestro>,
    time: &'scope RefCell<Time>,
) -> Reg {
    put(
        table,
        "DefineSnapshot",
        scope.create_function(|_, (name, groups): (String, Table)| {
            let mut patches = Vec::new();
            for pair in groups.pairs::<String, Table>() {
                let (group, fields) = pair?;
                patches.push((group, patch(&fields)?));
            }
            audio
                .borrow_mut()
                .define_snapshot(&name, &patches)
                .map_err(err)
        }),
    )?;
    put(
        table,
        "TransitionToSnapshot",
        scope.create_function(|_, (name, seconds): (String, f64)| {
            let now = time.borrow().unscaled_time;
            audio
                .borrow_mut()
                .transition_to_snapshot(&name, seconds, now)
                .map_err(err)
        }),
    )?;
    put(
        table,
        "GetSnapshot",
        scope.create_function(|_, ()| {
            let maestro = audio.borrow();
            Ok(maestro
                .mixer()
                .snapshot()
                .map(|(n, t)| (n.to_string(), t))
                .unzip())
        }),
    )?;
    put(
        table,
        "AddDuck",
        scope.create_function(|_, args: (String, String, f32, Option<f32>, Option<f32>)| {
            let (trigger, target, volume, attack, release) = args;
            let (attack, release) = (
                attack.unwrap_or(DUCK_ATTACK),
                release.unwrap_or(DUCK_RELEASE),
            );
            audio
                .borrow_mut()
                .add_duck(&trigger, &target, volume, attack, release)
                .map_err(err)
        }),
    )?;
    put(
        table,
        "ClearDucks",
        scope.create_function(|_, ()| {
            audio.borrow_mut().clear_ducks();
            Ok(())
        }),
    )
}

/// A snapshot entry `{ volume = 0.5, low_pass = 800, ... }` as a patch. Unknown
/// fields, and a resonance without its cutoff, are errors.
fn patch(fields: &Table) -> mlua::Result<GroupPatch> {
    for pair in fields.clone().pairs::<String, mlua::Value>() {
        let (key, _) = pair?;
        if !PATCH_FIELDS.contains(&key.as_str()) {
            let valid = PATCH_FIELDS.join(", ");
            return Err(err(format!(
                "snapshot field '{key}' unknown (fields: {valid})"
            )));
        }
    }
    let filter = |cutoff: &str, q: &str| -> mlua::Result<Option<Filter>> {
        let (hz, res): (Option<f32>, Option<f32>) = (fields.get(cutoff)?, fields.get(q)?);
        match (hz, res) {
            (Some(hz), res) => Ok(Some(Filter::new(hz, res.unwrap_or(0.0)))),
            (None, Some(_)) => Err(err(format!("snapshot sets {q} without {cutoff}"))),
            (None, None) => Ok(None),
        }
    };
    Ok(GroupPatch {
        volume: fields.get("volume")?,
        mute: fields.get("mute")?,
        low_pass: filter("low_pass", "low_pass_resonance")?,
        high_pass: filter("high_pass", "high_pass_resonance")?,
        reverb_send: fields.get("reverb_send")?,
    })
}

/// A group's live state as the table `GetGroupState` returns.
fn state_table<'lua>(lua: &'lua Lua, state: &GroupState) -> mlua::Result<Table<'lua>> {
    let s = &state.settings;
    let t = lua.create_table()?;
    t.set("name", state.name.as_str())?;
    t.set("parent", state.parent.as_deref())?;
    t.set("volume", s.volume)?;
    t.set("mute", s.mute)?;
    t.set("low_pass", s.low_pass.cutoff)?;
    t.set("low_pass_resonance", s.low_pass.resonance)?;
    t.set("high_pass", s.high_pass.cutoff)?;
    t.set("high_pass_resonance", s.high_pass.resonance)?;
    t.set("reverb_send", s.reverb_send)?;
    t.set("duck", state.duck)?;
    t.set("effective_volume", state.effective_volume)?;
    Ok(t)
}
