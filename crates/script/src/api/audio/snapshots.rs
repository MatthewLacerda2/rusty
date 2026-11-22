use mlua::Table;

use super::super::{put, Reg};
use super::effects::field_list;
use super::groups::err;
use crate::api::ApiCell;
use rusty_audio::mixer::{Filter, GroupPatch};
use rusty_audio::AudioMaestro;
use rusty_time::Time;

/// A duck's default attack and release, in seconds.
const DUCK_ATTACK: f32 = 0.1;
const DUCK_RELEASE: f32 = 0.5;

const PATCH_FIELDS: [&str; 8] = [
    "effects",
    "volume",
    "mute",
    "low_pass",
    "low_pass_resonance",
    "high_pass",
    "high_pass_resonance",
    "reverb_send",
];

pub(super) fn register_snapshots(
    lua: &mlua::Lua,
    table: &Table,
    audio: ApiCell<AudioMaestro>,
    time: ApiCell<Time>,
) -> Reg {
    put(
        table,
        "DefineSnapshot",
        lua.create_function(move |lua, (name, groups): (String, Table)| {
            let audio = audio.get(lua)?;
            let mut patches = Vec::new();
            for pair in groups.pairs::<String, Table>() {
                let (group, fields) = pair?;
                let patch = group_patch(&audio.borrow(), &group, &fields)?;
                patches.push((group, patch));
            }
            audio
                .borrow_mut()
                .define_snapshot(&name, &patches)
                .map_err(err)?;
            Ok(())
        }),
    )?;
    put(
        table,
        "TransitionToSnapshot",
        lua.create_function(move |lua, (name, seconds): (String, f64)| {
            time.with(lua, |time| {
                // The clock `app::audio::advance_mixer` steps blends on: the two must match.
                let now = time.borrow().unscaled_time;
                audio
                    .get(lua)?
                    .borrow_mut()
                    .transition_to_snapshot(&name, seconds, now)
                    .map_err(err)
            })
        }),
    )?;
    put(
        table,
        "GetSnapshot",
        lua.create_function(move |lua, ()| {
            let audio = &*audio.get(lua)?;
            let maestro = audio.borrow();
            Ok(maestro
                .mixer()
                .snapshot()
                .map(|(n, t)| (n.to_string(), t))
                .unzip())
        }),
    )?;
    register_ducks(lua, table, audio)
}

fn register_ducks(lua: &mlua::Lua, table: &Table, audio: ApiCell<AudioMaestro>) -> Reg {
    put(
        table,
        "AddDuck",
        lua.create_function(
            move |lua, args: (String, String, f32, Option<f32>, Option<f32>)| {
                let (trigger, target, volume, attack, release) = args;
                let (attack, release) = (
                    attack.unwrap_or(DUCK_ATTACK),
                    release.unwrap_or(DUCK_RELEASE),
                );
                audio
                    .get(lua)?
                    .borrow_mut()
                    .add_duck(&trigger, &target, volume, attack, release)
                    .map_err(err)
            },
        ),
    )?;
    put(
        table,
        "ClearDucks",
        lua.create_function(move |lua, ()| {
            audio.get(lua)?.borrow_mut().clear_ducks();
            Ok(())
        }),
    )
}

fn group_patch(maestro: &AudioMaestro, group: &str, fields: &Table) -> mlua::Result<GroupPatch> {
    let mut patch = patch(fields)?;
    if let Some(list) = fields.get::<Option<Table>>("effects")? {
        let tables = field_list(&list, "effects")?;
        let mixer = maestro.mixer();
        let id = mixer.require(group).map_err(err)?;
        patch.effect_params = Some(mixer.parse_effect_patch(id, &tables).map_err(err)?);
    }
    Ok(patch)
}

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
        ..GroupPatch::default()
    })
}
