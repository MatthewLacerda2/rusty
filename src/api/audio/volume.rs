//! src/api/audio/volume.rs — `Audio.SetVolume` / `GetVolume`: an entity's
//! AudioSource volume, the stored field and the live voice together.

use std::cell::RefCell;

use super::super::{put, Reg};
use crate::api::entity_arg::EntityArg;
use crate::audio::AudioMaestro;
use crate::scene::authoring::audio::source as audio_ops;
use crate::scene::Scene;

/// `SetVolume` / `GetVolume` on an entity's AudioSource.
pub(super) fn register<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
    audio: &'scope RefCell<AudioMaestro>,
) -> Reg {
    // Unity's `AudioSource.volume`: entity `id`'s source volume, through the op the
    // inspector writes (clamped to `[0, 1]`), then the live voice retuned to it.
    put(
        table,
        "SetVolume",
        scope.create_function(|_, (id, v): (EntityArg, f32)| {
            let id = id.live("Audio", "SetVolume")?;
            let stored = scene.borrow_mut().world.audio_mut(id).map(|mut a| {
                audio_ops::set_volume(&mut a, v);
                a.volume
            });
            if let Some(v) = stored {
                audio.borrow_mut().set_source_volume(id, v);
            }
            Ok(())
        }),
    )?;

    put(
        table,
        "GetVolume",
        scope.create_function(|_, id: EntityArg| {
            Ok(scene
                .borrow()
                .world
                .audio(id.read())
                .map_or(0.0, |a| a.volume))
        }),
    )
}
