//! src/api/audio/occlusion.rs — the `Audio` occlusion verbs (#467).
//!
//! The global settings (strength, the layers that occlude, the per-tick cast
//! budget) and the per-source opt-out (`AudioSource.occlusion_enabled`). The
//! factor itself is read back through `Audio.GetSpatial`.

use std::cell::RefCell;

use mlua::{Table, Value};

use super::super::{put, Reg};
use super::groups::err;
use crate::audio::{AudioMaestro, OcclusionSettings};
use crate::scene::authoring::audio as audio_ops;
use crate::scene::Scene;

/// The keys `SetOcclusionSettings` takes.
const KEYS: [&str; 3] = ["strength", "layer_mask", "voices_per_tick"];

/// `settings` with the fields `patch` names replaced; an unknown key is an error.
fn patched(mut settings: OcclusionSettings, patch: &Table) -> mlua::Result<OcclusionSettings> {
    for pair in patch.clone().pairs::<String, Value>() {
        let (key, value) = pair?;
        let number = |v: Value| -> mlua::Result<f64> {
            match v {
                Value::Integer(i) => Ok(i as f64),
                Value::Number(n) => Ok(n),
                _ => Err(err(format!("occlusion '{key}' must be a number"))),
            }
        };
        match key.as_str() {
            "strength" => settings.strength = number(value)? as f32,
            // A mask keeps its low 32 bits, so `~(1 << 8)` means "all but layer 8".
            "layer_mask" => match value {
                Value::Integer(i) => settings.layer_mask = i as u32,
                v => settings.layer_mask = number(v)? as u32,
            },
            "voices_per_tick" => settings.voices_per_tick = number(value)? as u32,
            _ => {
                let valid = KEYS.join(", ");
                return Err(err(format!("unknown occlusion setting '{key}' ({valid})")));
            }
        }
    }
    Ok(settings)
}

/// `SetOcclusionSettings` / `GetOcclusionSettings` and the per-source toggle.
pub(super) fn register_occlusion<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &Table,
    scene: &'scope RefCell<Scene>,
    audio: &'scope RefCell<AudioMaestro>,
) -> Reg {
    put(
        table,
        "SetOcclusionSettings",
        scope.create_function(|_, patch: Table| {
            let settings = patched(audio.borrow().occlusion_settings(), &patch)?;
            audio.borrow_mut().set_occlusion_settings(settings);
            Ok(())
        }),
    )?;
    put(
        table,
        "GetOcclusionSettings",
        scope.create_function(|lua, ()| {
            let s = audio.borrow().occlusion_settings();
            let t = lua.create_table()?;
            t.set("strength", s.strength)?;
            t.set("layer_mask", s.layer_mask)?;
            t.set("voices_per_tick", s.voices_per_tick)?;
            Ok(t)
        }),
    )?;
    put(
        table,
        "SetOcclusionEnabled",
        scope.create_function(|_, (id, on): (u32, bool)| {
            let mut scene = scene.borrow_mut();
            let mut source = scene
                .world
                .audio_mut(id)
                .ok_or_else(|| err(format!("entity {id} has no AudioSource")))?;
            audio_ops::set_occlusion_enabled(&mut source, on);
            Ok(())
        }),
    )?;
    put(
        table,
        "GetOcclusionEnabled",
        scope.create_function(|_, id: u32| {
            let scene = scene.borrow();
            Ok(scene.world.audio(id).map(|a| a.occlusion_enabled))
        }),
    )
}
