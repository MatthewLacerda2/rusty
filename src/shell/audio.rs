//! src/shell/audio.rs — apply the audio mix to the device once a frame (#412).
//!
//! After the sim advances, every live voice is re-resolved against the active camera
//! and the clock state (`Time.timeScale`, `Time.Pause`) and the resulting mix is
//! handed to the backend: distance rolloff, pan and time scaling reach the speakers.
//! It runs in the shell, so the editor and the standalone player both hear it, and
//! it uses the same listener and emitter lookups as `Audio.GetSpatial`, so what the
//! agent reads back is what plays.

use crate::api::audio::{emitter, listener};
use crate::app::GameWorld;
use crate::audio::MixEnv;

/// Push this frame's mix for every live voice to the audio backend.
pub fn apply_mix(game: &GameWorld) {
    let env = {
        let time = game.time().borrow();
        MixEnv {
            listener: listener(&game.resources.camera),
            time_scale: time.time_scale,
            paused: time.paused,
        }
    };
    let scene = game.scene().borrow();
    game.resources
        .audio
        .borrow_mut()
        .apply_mix(&env, |id| emitter(&scene, id));
}

#[cfg(test)]
#[path = "audio_tests.rs"]
mod audio_tests;
