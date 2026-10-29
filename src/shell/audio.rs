//! src/shell/audio.rs — apply the audio mix to the device once a frame (#412).
//!
//! Every `GameWorld::tick` ends by pushing the mix and clocking the output (#1579),
//! but a paused frame with no `Time.Step` queued runs no tick. So the shell pushes
//! the mix once more each frame, advancing no time: a `Time.Pause` from the console
//! reaches the device, and a capture drains what the device played meanwhile. The
//! editor and the standalone player both run it.

use crate::app::GameWorld;

/// Re-push this frame's mix for every live voice, advancing the output by nothing.
pub fn apply_mix(game: &GameWorld) {
    game.push_audio(0.0);
}

#[cfg(test)]
#[path = "audio_tests.rs"]
mod audio_tests;
