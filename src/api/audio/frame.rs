//! src/api/audio/frame.rs — the once-a-frame audio push (#412, #1579).
//!
//! After the sim advances, every live voice is re-resolved against the active camera
//! and the clock state (`Time.timeScale`, `Time.Pause`) and the resulting mix is
//! handed to the backend: distance rolloff, pan and time scaling reach the output.
//! Then the output is told how much time went by, so an offline mixer renders it and
//! a running capture takes it (`audio::capture`). The windowed shell calls this once
//! a frame and the headless harness once a tick, so a scenario's capture hears the
//! same mix the speakers would. It uses the same listener and emitter lookups as
//! `Audio.GetSpatial`, so what the agent reads back is what plays.

use std::cell::RefCell;

use super::{emitter, listener};
use crate::audio::{AudioMaestro, MixEnv};
use crate::scene::{Camera, Scene};
use crate::time::Time;

/// Push this frame's mix for every live voice, then advance the output `seconds`.
pub fn push_frame(
    scene: &RefCell<Scene>,
    audio: &RefCell<AudioMaestro>,
    time: &RefCell<Time>,
    camera: &RefCell<Camera>,
    seconds: f64,
) {
    let env = {
        let time = time.borrow();
        MixEnv {
            listener: listener(camera),
            time_scale: time.time_scale,
            paused: time.paused,
        }
    };
    let scene = scene.borrow();
    let mut audio = audio.borrow_mut();
    audio.apply_mix(&env, |id| emitter(&scene, id));
    audio.advance_output(seconds);
}
