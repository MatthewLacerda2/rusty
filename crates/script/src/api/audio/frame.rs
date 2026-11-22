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
            // The scaled sim time doppler measures motion over; a push that ran no tick has none.
            dt: if seconds > 0.0 { time.delta_time } else { 0.0 },
        }
    };
    let mut audio = audio.borrow_mut();
    audio.apply_mix(&env, |id| emitter(&scene, id));
    audio.advance_output(seconds, env.paused);
}
