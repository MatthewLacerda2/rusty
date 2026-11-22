use std::cell::RefCell;

use mlua::Lua;

mod doppler;
mod effects;
mod frame;
mod groups;
mod occlusion;
mod reverb;
mod snapshots;
mod volume;

pub use frame::push_frame;

use super::{put, Reg};
use crate::api::entity_arg::EntityArg;
use crate::api::{ApiCell, ApiCtx};
use glam::Vec3;

use rusty_audio::{AudioMaestro, Listener, Rolloff, Shot, SpeakerMode};
use rusty_components::AudioSourceComponent;
use rusty_scene::Camera;
use rusty_scene::Scene;
use rusty_time::Time;

pub fn register(lua: &Lua, ctx: &ApiCtx) -> Reg {
    let (scene, audio, time, camera) = (ctx.scene, ctx.audio, ctx.time, ctx.camera);
    let table = lua.create_table().map_err(|e| e.to_string())?;

    register_source_control(lua, &table, scene, audio, time)?;
    register_oneshot_and_master(lua, &table, audio, time)?;
    register_spatial(lua, &table, scene, audio, camera)?;
    register_speaker_mode(lua, &table, audio)?;
    groups::register_groups(lua, &table, audio)?;
    groups::register_output(lua, &table, scene, audio)?;
    effects::register_effects(lua, &table, audio)?;
    snapshots::register_snapshots(lua, &table, audio, time)?;
    occlusion::register_occlusion(lua, &table, scene, audio)?;
    doppler::register_doppler(lua, &table, scene, audio)?;
    doppler::register_default_speed(lua, &table, ctx.application)?;
    reverb::register_reverb(lua, &table, audio)?;

    lua.globals().set("Audio", table).map_err(|e| e.to_string())
}

pub fn listener(camera: &RefCell<Camera>, scene: &Scene) -> Listener {
    let cam = rusty_scene::game_camera_from_scene(&camera.borrow(), scene);
    Listener::new(cam.position, cam.right())
}

pub fn emitter(scene: &Scene, id: u32) -> Option<(AudioSourceComponent, Vec3)> {
    let src = scene.world.audio(id)?;
    Some((src.clone(), scene.world.world_position(id)))
}

fn tick(time: &RefCell<Time>) -> u64 {
    time.borrow().frame_count
}

fn source_clip_pos(scene: &RefCell<Scene>, id: u32) -> Option<(String, [f32; 3])> {
    let scene = scene.borrow();
    let src = scene.world.audio(id)?;
    let p = scene.world.world_position(id);
    Some((src.clip.clone(), [p.x, p.y, p.z]))
}

fn register_source_control(
    lua: &mlua::Lua,
    table: &mlua::Table,
    scene: ApiCell<Scene>,
    audio: ApiCell<AudioMaestro>,
    time: ApiCell<Time>,
) -> Reg {
    put(
        table,
        "Play",
        lua.create_function(move |lua, id: EntityArg| {
            let scene = &*scene.get(lua)?;
            let audio = &*audio.get(lua)?;
            let id = id.live("Audio", "Play")?;
            let now = tick(&*time.get(lua)?);
            let src = scene.borrow().world.audio(id).map(|c| c.clone());
            let pos = src
                .as_ref()
                .and_then(|_| source_clip_pos(scene, id))
                .map(|(_, p)| p)
                .unwrap_or([0.0; 3]);
            let played = src
                .map(|s| audio.borrow_mut().play_source(id, &s, pos, now))
                .unwrap_or(false);
            Ok(played)
        }),
    )?;

    put(
        table,
        "Stop",
        lua.create_function(move |lua, id: EntityArg| {
            let id = id.live("Audio", "Stop")?;
            let now = tick(&*time.get(lua)?);
            let pos = source_clip_pos(&*scene.get(lua)?, id)
                .map(|(_, p)| p)
                .unwrap_or([0.0; 3]);
            audio.get(lua)?.borrow_mut().stop_source(id, pos, now, true);
            Ok(())
        }),
    )?;
    volume::register(lua, table, scene, audio)
}

type PlayAtArgs = (
    String,
    f32,
    f32,
    f32,
    Option<f32>,
    Option<f32>,
    Option<f32>,
    Option<String>,
);

fn rolloff_arg(min: Option<f32>, max: Option<f32>) -> Result<Rolloff, String> {
    let (min_distance, max_distance) = match (min, max) {
        (None, None) => return Ok(Rolloff::default()),
        (Some(min), Some(max)) => (min, max),
        _ => return Err("Audio.PlayAt: pass both min_distance and max_distance".into()),
    };
    if !(min_distance >= 0.0 && max_distance >= min_distance && max_distance.is_finite()) {
        return Err(format!(
            "Audio.PlayAt: rolloff band needs 0 <= min_distance <= max_distance, \
             got {min_distance} .. {max_distance}"
        ));
    }
    Ok(Rolloff {
        min_distance,
        max_distance,
    })
}

fn register_oneshot_and_master(
    lua: &mlua::Lua,
    table: &mlua::Table,
    audio: ApiCell<AudioMaestro>,
    time: ApiCell<Time>,
) -> Reg {
    put(
        table,
        "PlayAt",
        lua.create_function(move |lua, args: PlayAtArgs| {
            let (path, x, y, z, vol, min, max, group) = args;
            let rolloff = rolloff_arg(min, max).map_err(mlua::Error::RuntimeError)?;
            let now = tick(&*time.get(lua)?);
            let shot = Shot {
                volume: vol.unwrap_or(1.0),
                rolloff,
                group: group.as_deref().unwrap_or(""),
                ..Shot::new(&path, [x, y, z])
            };
            let played = audio.get(lua)?.borrow_mut().play_at(&shot, 0, now);
            Ok(played)
        }),
    )?;

    put(
        table,
        "GetMasterVolume",
        lua.create_function(move |lua, ()| Ok(audio.get(lua)?.borrow().master_volume())),
    )?;

    put(
        table,
        "SetMasterVolume",
        lua.create_function(move |lua, v: f32| {
            audio.get(lua)?.borrow_mut().set_master_volume(v);
            Ok(())
        }),
    )
}

/// `GetSpeakerMode` / `SetSpeakerMode` — the output profile (`headphones` / `tv` /
/// `home_theater`). Output shaping only: the sim never reads it.
fn register_speaker_mode(
    lua: &mlua::Lua,
    table: &mlua::Table,
    audio: ApiCell<AudioMaestro>,
) -> Reg {
    put(
        table,
        "GetSpeakerMode",
        lua.create_function(move |lua, ()| Ok(audio.get(lua)?.borrow().speaker_mode().name())),
    )?;

    put(
        table,
        "SetSpeakerMode",
        lua.create_function(move |lua, name: String| {
            let mode = SpeakerMode::parse(&name).ok_or_else(|| {
                let valid: Vec<_> = SpeakerMode::ALL.iter().map(|m| m.name()).collect();
                mlua::Error::RuntimeError(format!(
                    "Audio.SetSpeakerMode: unknown mode '{name}' (expected one of {})",
                    valid.join(", ")
                ))
            })?;
            audio.get(lua)?.borrow_mut().set_speaker_mode(mode);
            Ok(())
        }),
    )
}

fn register_spatial(
    lua: &mlua::Lua,
    table: &mlua::Table,
    scene: ApiCell<Scene>,
    audio: ApiCell<AudioMaestro>,
    camera: ApiCell<Camera>,
) -> Reg {
    put(
        table,
        "GetSpatial",
        lua.create_function(move |lua, id: EntityArg| {
            let scene = &*scene.get(lua)?;
            let audio = &*audio.get(lua)?;
            let id = id.read();
            let scene = &scene.borrow();
            let Some((src, pos)) = emitter(scene, id) else {
                return Ok((0.0_f32, 0.0_f32, false, 0.0_f32));
            };
            let info =
                audio
                    .borrow()
                    .voice_info(id, &src, pos, &listener(&*camera.get(lua)?, scene));
            let spatial = info.spatial;
            Ok((spatial.gain, spatial.pan, info.playing, info.occlusion))
        }),
    )
}
