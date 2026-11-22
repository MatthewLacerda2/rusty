use std::cell::RefCell;

use mlua::{Lua, Table};

use super::super::{put, Reg};
use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use rusty_audio::mixer::{Filter, GroupPatch, GroupState};
use rusty_audio::AudioMaestro;
use rusty_edit::authoring::audio::source as audio_ops;
use rusty_scene::Scene;

pub(super) fn err(e: String) -> mlua::Error {
    mlua::Error::RuntimeError(format!("Audio: {e}"))
}

fn set(audio: &RefCell<AudioMaestro>, name: &str, patch: GroupPatch) -> mlua::Result<()> {
    audio.borrow_mut().set_group(name, &patch).map_err(err)
}

pub(super) fn register_groups(lua: &mlua::Lua, table: &Table, audio: ApiCell<AudioMaestro>) -> Reg {
    put(
        table,
        "GetGroups",
        lua.create_function(move |lua, ()| {
            let audio = &*audio.get(lua)?;
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
        lua.create_function(move |lua, (name, parent): (String, Option<String>)| {
            audio
                .get(lua)?
                .borrow_mut()
                .create_group(&name, parent.as_deref())
                .map_err(err)
        }),
    )?;
    put(
        table,
        "GetGroupState",
        lua.create_function(move |lua, name: String| {
            let audio = &*audio.get(lua)?;
            let maestro = audio.borrow();
            let id = maestro.mixer().require(&name).map_err(err)?;
            state_table(lua, &maestro.mixer().state(id))
        }),
    )?;
    register_levels(lua, table, audio)?;
    register_filters(lua, table, audio)
}

fn register_levels(lua: &mlua::Lua, table: &Table, audio: ApiCell<AudioMaestro>) -> Reg {
    put(
        table,
        "SetGroupVolume",
        lua.create_function(move |lua, (name, v): (String, f32)| {
            set(
                &*audio.get(lua)?,
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
        lua.create_function(move |lua, (name, mute): (String, bool)| {
            set(
                &*audio.get(lua)?,
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
        lua.create_function(move |lua, (name, v): (String, f32)| {
            set(
                &*audio.get(lua)?,
                &name,
                GroupPatch {
                    reverb_send: Some(v),
                    ..Default::default()
                },
            )
        }),
    )
}

fn register_filters(lua: &mlua::Lua, table: &Table, audio: ApiCell<AudioMaestro>) -> Reg {
    put(
        table,
        "SetGroupLowPass",
        lua.create_function(move |lua, (name, hz, q): (String, f32, Option<f32>)| {
            let low_pass = Some(Filter::new(hz, q.unwrap_or(0.0)));
            set(
                &*audio.get(lua)?,
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
        lua.create_function(move |lua, (name, hz, q): (String, f32, Option<f32>)| {
            let high_pass = Some(Filter::new(hz, q.unwrap_or(0.0)));
            set(
                &*audio.get(lua)?,
                &name,
                GroupPatch {
                    high_pass,
                    ..Default::default()
                },
            )
        }),
    )
}

pub(super) fn register_output(
    lua: &mlua::Lua,
    table: &Table,
    scene: ApiCell<Scene>,
    audio: ApiCell<AudioMaestro>,
) -> Reg {
    put(
        table,
        "SetOutputGroup",
        lua.create_function(move |lua, (id, group): (EntityArg, String)| {
            let scene = &*scene.get(lua)?;
            let id = id.live("Audio", "SetOutputGroup")?;
            if !group.is_empty() {
                audio
                    .get(lua)?
                    .borrow()
                    .mixer()
                    .require(&group)
                    .map_err(err)?;
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
        lua.create_function(move |lua, id: EntityArg| {
            let scene = &*scene.get(lua)?;
            let scene = scene.borrow();
            Ok(scene.world.audio(id.read()).map(|a| a.output_group.clone()))
        }),
    )
}

fn state_table(lua: &Lua, state: &GroupState) -> mlua::Result<Table> {
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
