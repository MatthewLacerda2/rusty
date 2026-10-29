//! src/shell/audio.rs — apply the audio mix to the device once a frame (#412).
//!
//! After the sim advances, every live voice is re-resolved against the active camera
//! and the clock state and handed to the backend, and the output learns how long the
//! frame was (a running capture drains it, #1579). The work is
//! `api::audio::push_frame`, shared with the headless harness; this is the shell's
//! call, so the editor and the standalone player both hear it.

use crate::app::GameWorld;

/// Push this frame's mix for every live voice to the audio backend; `seconds` is
/// the frame's real length.
pub fn apply_mix(game: &GameWorld, seconds: f64) {
    crate::api::audio::push_frame(
        game.scene(),
        &game.resources.audio,
        game.time(),
        &game.resources.camera,
        seconds,
    );
}

#[cfg(test)]
#[path = "audio_tests.rs"]
mod audio_tests;
