//! crates/script/src/api/audio/volume.rs — `Audio.SetVolume` / `GetVolume`: an entity's
//! AudioSource volume, the stored field and the live voice together.

use super::super::{put, Reg};
use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use rusty_audio::AudioMaestro;
use rusty_scene::authoring::audio::source as audio_ops;
use rusty_scene::Scene;

/// `SetVolume` / `GetVolume` on an entity's AudioSource.
pub(super) fn register(
    lua: &mlua::Lua,
    table: &mlua::Table,
    scene: ApiCell<Scene>,
    audio: ApiCell<AudioMaestro>,
) -> Reg {
    // Unity's `AudioSource.volume`: entity `id`'s source volume, through the op the
    // inspector writes (clamped to `[0, 1]`), then the live voice retuned to it.
    put(
        table,
        "SetVolume",
        lua.create_function(move |lua, (id, v): (EntityArg, f32)| {
            scene.with(lua, |scene| {
                let id = id.live("Audio", "SetVolume")?;
                let stored = scene.borrow_mut().world.audio_mut(id).map(|mut a| {
                    audio_ops::set_volume(&mut a, v);
                    a.volume
                });
                if let Some(v) = stored {
                    audio.get(lua)?.borrow_mut().set_source_volume(id, v);
                }
                Ok(())
            })
        }),
    )?;

    put(
        table,
        "GetVolume",
        lua.create_function(move |lua, id: EntityArg| {
            Ok(scene
                .get(lua)?
                .borrow()
                .world
                .audio(id.read())
                .map_or(0.0, |a| a.volume))
        }),
    )
}
