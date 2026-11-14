//! crates/script/src/api/audio/frame.rs — the once-a-frame audio push (#412, #1579).
//!
//! After the sim advances, every live voice is re-resolved against the active camera
//! and the clock state (`Time.timeScale`, `Time.Pause`) and the resulting mix is
//! handed to the backend: distance rolloff, pan, time scaling and doppler (#1749)
//! reach the output.
//! Then the output is told how much time went by, so an offline mixer renders it and
//! a running capture takes it (`audio::capture`). `GameWorld::push_audio` calls it at
//! the end of every tick, whoever drives the sim, so a scenario's capture hears the
//! same mix the speakers would, and once on a paused frame that ran no tick, with no
//! time, so a `Time.Pause` reaches the device. It uses the same listener and emitter lookups as
//! `Audio.GetSpatial`, so what the agent reads back is the mix the device is
//! given: the target it glides to over a few ms (#1693), not the sample playing.

use std::cell::RefCell;

use super::{emitter, listener};
use rusty_audio::{AudioMaestro, MixEnv};
use rusty_scene::{Camera, Scene};
use rusty_time::Time;

/// Push this frame's mix for every live voice, then advance the output `seconds`.
pub fn push_frame(
    scene: &RefCell<Scene>,
    audio: &RefCell<AudioMaestro>,
    time: &RefCell<Time>,
    camera: &RefCell<Camera>,
    seconds: f64,
) {
    let scene = scene.borrow();
    let env = {
        let time = time.borrow();
        MixEnv {
            listener: listener(camera, &scene),
            time_scale: time.time_scale,
            paused: time.paused,
            // The scaled sim time this tick ran, which doppler measures motion over.
            dt: if seconds > 0.0 { time.delta_time } else { 0.0 },
        }
    };
    let mut audio = audio.borrow_mut();
    audio.apply_mix(&env, |id| emitter(&scene, id));
    audio.advance_output(seconds, env.paused);
}
