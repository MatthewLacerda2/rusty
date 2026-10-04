//! src/api/audio/snapshots.rs — the `Audio` snapshot and duck verbs (#465).
//!
//! Snapshots are named partial sets of group settings a script blends to over sim
//! time (`Time.unscaledTime`, so a blend runs through slow-mo); ducks lower one
//! group while another plays. Unknown snapshot, group or field names are errors.

use std::cell::RefCell;

use mlua::Table;

use super::super::{put, Reg};
use super::groups::err;
use crate::audio::mixer::{Filter, GroupPatch};
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

pub(super) fn register_snapshots<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &Table,
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
    register_ducks(scope, table, audio)
}

/// `AddDuck` / `ClearDucks`.
fn register_ducks<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &Table,
    audio: &'scope RefCell<AudioMaestro>,
) -> Reg {
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
