//! src/api/audio.rs — `Audio` namespace (#212).
//!
//! Script/REPL/bot control over the engine's audio: start/stop an entity's
//! `AudioSource` (`Play`/`Stop`), retune its volume (`SetVolume`), fire a
//! fire-and-forget one-shot at a world position (`PlayAt`), and read/set the single
//! master volume on the `AudioMaestro`. One surface, three callers — the same verbs
//! the editor's play-state and the play-mode systems drive.
//!
//! Every verb routes through the shared `AudioMaestro` (a Resource), so a scripted
//! play and `play_on_start` end up in one device + one introspection log. The clip
//! and world position for an entity `AudioSource` are read from the live scene; the
//! action's `tick` is the deterministic frame count (`Time.frameCount`), so the log
//! orders without any wall clock.

use std::cell::RefCell;

use mlua::Lua;

use super::{put, Reg};
use crate::audio::{AudioMaestro, Listener};
use crate::render::Camera;
use crate::scene::Scene;
use crate::time::Time;

/// Register the `Audio` namespace onto `lua`.
pub fn register<'lua, 'scope>(
    lua: &'lua Lua,
    scope: &mlua::Scope<'lua, 'scope>,
    scene: &'scope RefCell<Scene>,
    audio: &'scope RefCell<AudioMaestro>,
    time: &'scope RefCell<Time>,
    camera: &'scope RefCell<Camera>,
) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;

    register_source_control(scope, &table, scene, audio, time)?;
    register_oneshot_and_master(scope, &table, audio, time)?;
    register_spatial(scope, &table, scene, audio, camera)?;

    lua.globals().set("Audio", table).map_err(|e| e.to_string())
}

/// The listener — the active camera — as the spatial math wants it (position + right).
fn listener(camera: &RefCell<Camera>) -> Listener {
    let cam = camera.borrow();
    Listener::new(cam.position, cam.right())
}

/// The current play-mode frame, used as the deterministic event `tick`.
fn tick(time: &RefCell<Time>) -> u64 {
    time.borrow().frame_count
}

/// An entity's `(clip, world-position)` for the audio log, read from the live scene.
/// Returns `None` when the entity has no `AudioSource`.
fn source_clip_pos(scene: &RefCell<Scene>, id: u32) -> Option<(String, [f32; 3])> {
    let scene = scene.borrow();
    let e = scene.get_entity(id)?;
    let src = e.audio.as_ref()?;
    let p = e.transform.position;
    Some((src.clip.clone(), [p.x, p.y, p.z]))
}

/// `Play` / `Stop` / `SetVolume` — drive an entity's own `AudioSource`.
fn register_source_control<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
    audio: &'scope RefCell<AudioMaestro>,
    time: &'scope RefCell<Time>,
) -> Reg {
    // Start (or restart) entity `id`'s AudioSource from the start. Returns whether
    // the entity has a source (the backend may still be silent on a null device).
    put(
        table,
        "Play",
        scope.create_function(|_, id: u32| {
            let now = tick(time);
            let src = scene.borrow().get_entity(id).and_then(|e| e.audio.clone());
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

    // Stop entity `id`'s voice (logs a Stop event).
    put(
        table,
        "Stop",
        scope.create_function(|_, id: u32| {
            let now = tick(time);
            let pos = source_clip_pos(scene, id)
                .map(|(_, p)| p)
                .unwrap_or([0.0; 3]);
            audio.borrow_mut().stop_source(id, pos, now, true);
            Ok(())
        }),
    )?;

    // Retune entity `id`'s live voice volume (pre-master). No-op if not playing.
    put(
        table,
        "SetVolume",
        scope.create_function(|_, (id, v): (u32, f32)| {
            audio.borrow_mut().set_source_volume(id, v);
            Ok(())
        }),
    )
}

/// `PlayAt` (fire-and-forget one-shot) + master-volume get/set.
fn register_oneshot_and_master<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &mlua::Table,
    audio: &'scope RefCell<AudioMaestro>,
    time: &'scope RefCell<Time>,
) -> Reg {
    // Fire a one-shot at a world position. `vol` is optional (defaults to 1.0).
    // Leaves no persistent component — its only trace is the maestro's event log.
    put(
        table,
        "PlayAt",
        scope.create_function(
            |_, (path, x, y, z, vol): (String, f32, f32, f32, Option<f32>)| {
                let now = tick(time);
                let played =
                    audio
                        .borrow_mut()
                        .play_at(&path, [x, y, z], vol.unwrap_or(1.0), 0, now);
                Ok(played)
            },
        ),
    )?;

    put(
        table,
        "GetMasterVolume",
        scope.create_function(|_, ()| Ok(audio.borrow().master_volume())),
    )?;

    put(
        table,
        "SetMasterVolume",
        scope.create_function(|_, v: f32| {
            audio.borrow_mut().set_master_volume(v);
            Ok(())
        }),
    )
}

/// `GetSpatial` — read the resolved spatial state of an entity's `AudioSource` against
/// the listener (the active camera). Returns `(gain, pan, playing)`: `gain` is the
/// pre-master volume after distance rolloff + `spatial_blend`, `pan` is `[-1, 1]`
/// (left→right). This is the agent-facing introspection (#213) for "how a source is
/// heard right now" — pure math, identical with or without an audio device.
fn register_spatial<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
    audio: &'scope RefCell<AudioMaestro>,
    camera: &'scope RefCell<Camera>,
) -> Reg {
    put(
        table,
        "GetSpatial",
        scope.create_function(|_, id: u32| {
            let Some((src, pos)) = ({
                let scene = scene.borrow();
                scene.get_entity(id).and_then(|e| {
                    let s = e.audio.as_ref()?.clone();
                    let p = e.transform.position;
                    Some((s, p))
                })
            }) else {
                // No source: report silent + centred + not playing.
                return Ok((0.0_f32, 0.0_f32, false));
            };
            let info = audio.borrow().voice_info(id, &src, pos, &listener(camera));
            Ok((info.spatial.gain, info.spatial.pan, info.playing))
        }),
    )
}
