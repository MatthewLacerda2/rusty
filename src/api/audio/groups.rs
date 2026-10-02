//! src/api/audio/groups.rs — the `Audio` mixer-group verbs (#465).
//!
//! Groups (buses) on the `AudioMaestro`'s mixer: list and create them, set their
//! volume / mute / filters / reverb send, read any group's live state back, and pick
//! the group an entity's `AudioSource` plays through (Unity's
//! `outputAudioMixerGroup`). Unknown group names are errors naming the groups that
//! exist, so a typo never fails silently.

use std::cell::RefCell;

use mlua::{Lua, Table};

use super::super::{put, Reg};
use crate::audio::mixer::{Filter, GroupPatch, GroupState};
use crate::audio::AudioMaestro;
use crate::scene::authoring::audio as audio_ops;
use crate::scene::Scene;

/// A mixer error as a Lua error.
pub(super) fn err(e: String) -> mlua::Error {
    mlua::Error::RuntimeError(format!("Audio: {e}"))
}

/// Apply `patch` to group `name`.
fn set(audio: &RefCell<AudioMaestro>, name: &str, patch: GroupPatch) -> mlua::Result<()> {
    audio.borrow_mut().set_group(name, &patch).map_err(err)
}

/// `GetGroups` / `CreateGroup` / `GetGroupState`, then the setters.
pub(super) fn register_groups<'lua, 'scope>(
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
        "GetGroupState",
        scope.create_function(|lua, name: String| {
            let maestro = audio.borrow();
            let id = maestro.mixer().require(&name).map_err(err)?;
            state_table(lua, &maestro.mixer().state(id))
        }),
    )?;
    register_levels(scope, table, audio)?;
    register_filters(scope, table, audio)
}

/// `SetGroupVolume` / `SetGroupMute` / `SetGroupReverbSend`.
fn register_levels<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &Table<'lua>,
    audio: &'scope RefCell<AudioMaestro>,
) -> Reg {
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
    )
}

/// `SetGroupLowPass` / `SetGroupHighPass`.
fn register_filters<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &Table<'lua>,
    audio: &'scope RefCell<AudioMaestro>,
) -> Reg {
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
    )
}

/// `SetOutputGroup` / `GetOutputGroup` — the mixer group an entity's `AudioSource`
/// plays through (`""` is Master). Read when its voice starts, so a change reroutes
/// the next `Play`.
pub(super) fn register_output<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &Table<'lua>,
    scene: &'scope RefCell<Scene>,
    audio: &'scope RefCell<AudioMaestro>,
) -> Reg {
    put(
        table,
        "SetOutputGroup",
        scope.create_function(|_, (id, group): (u32, String)| {
            if !group.is_empty() {
                audio.borrow().mixer().require(&group).map_err(err)?;
            }
            let mut scene = scene.borrow_mut();
            let mut source = scene
                .world
                .audio_mut(id)
                .ok_or_else(|| err(format!("entity {id} has no AudioSource")))?;
            audio_ops::set_output_group(&mut source, group);
            Ok(())
        }),
    )?;
    put(
        table,
        "GetOutputGroup",
        scope.create_function(|_, id: u32| {
            let scene = scene.borrow();
            Ok(scene.world.audio(id).map(|a| a.output_group.clone()))
        }),
    )
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
