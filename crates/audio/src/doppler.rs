use glam::Vec3;

use super::backend::VoiceMix;
use super::maestro::AudioMaestro;
use super::mix::MixEnv;
use rusty_components::AudioSourceComponent;

pub use rusty_core::settings::audio::SPEED_OF_SOUND;
// The playback-rate bounds a doppler shift is clamped to: an octave down and up.
pub const RATE_MIN: f32 = 0.5;
pub const RATE_MAX: f32 = 2.0;
/// The speed, in m/s, past which one tick's move is a teleport and counts as still.
/// Not the speed of sound: a body faster than sound plays at the clamp, and a slow
/// speed of sound (`Audio.SetSpeedOfSound`) must not silence real motion.
pub const TELEPORT_SPEED: f32 = 1000.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DopplerState {
    pub speed_of_sound: f32,
    pub listener_was: Option<Vec3>,
}

impl Default for DopplerState {
    fn default() -> Self {
        Self {
            speed_of_sound: SPEED_OF_SOUND,
            listener_was: None,
        }
    }
}

impl DopplerState {
    pub fn listen(&mut self, env: &MixEnv) -> Body {
        let now = env.listener.position;
        let was = self.listener_was.replace(now).unwrap_or(now);
        Body {
            position: now,
            velocity: velocity(was, now, env.dt),
        }
    }

    pub fn forget(&mut self) {
        self.listener_was = None;
    }
}

impl AudioMaestro {
    /// The speed of sound doppler resolves with, in m/s.
    pub fn speed_of_sound(&self) -> f32 {
        self.doppler.speed_of_sound
    }

    pub fn set_speed_of_sound(&mut self, c: f32) -> bool {
        let ok = c.is_finite() && c > 0.0;
        if ok {
            self.doppler.speed_of_sound = c;
        }
        ok
    }

    pub fn shift_origin(&mut self, by: Vec3) {
        if let Some(was) = self.doppler.listener_was.as_mut() {
            *was += by;
        }
        self.env.listener.position += by;
        for live in self.entity_voices.values_mut() {
            (live.position, live.heard_at) = (live.position + by, live.heard_at + by);
        }
        for shot in self.oneshots.values_mut() {
            shot.position += by;
        }
    }

    pub fn forget_motion(&mut self) {
        self.doppler.forget();
    }

    /// The doppler rate entity `id`'s voice plays at now (`1.0` when not playing).
    pub fn source_doppler(&self, id: u32) -> f32 {
        self.entity_voices
            .get(&id)
            .map_or(1.0, |live| live.mix.doppler)
    }
}

pub fn source_rate(
    env: &MixEnv,
    ear: Body,
    (was, now): (Vec3, Vec3),
    source: &AudioSourceComponent,
    c: f32,
    held: VoiceMix,
) -> f32 {
    if env.dt <= 0.0 {
        return if source.is_time_scaled {
            held.doppler
        } else {
            1.0
        };
    }
    let body = Body {
        position: now,
        velocity: velocity(was, now, env.dt),
    };
    let level = source.doppler_level * source.spatial_blend.clamp(0.0, 1.0);
    rate(ear, body, c, level)
}

pub fn velocity(was: Vec3, now: Vec3, dt: f32) -> Vec3 {
    let v = (now - was) / dt;
    if dt > 0.0 && v.is_finite() && v.length() < TELEPORT_SPEED {
        v
    } else {
        Vec3::ZERO
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Body {
    pub position: Vec3,
    pub velocity: Vec3,
}

pub fn rate(listener: Body, source: Body, c: f32, level: f32) -> f32 {
    let Some(n) = (listener.position - source.position).try_normalize() else {
        return 1.0;
    };
    if level <= 0.0 || c <= 0.0 {
        return 1.0;
    }
    let along = |v: Vec3| (v * level).dot(n);
    let (heard, sent) = (along(listener.velocity), along(source.velocity));
    // Floored above 0: a source at or past the speed of sound would divide by zero or flip sign.
    let r = (c - heard) / (c - sent).max(c * 1e-3);
    if heard.is_finite() && sent.is_finite() {
        r.clamp(RATE_MIN, RATE_MAX)
    } else {
        1.0
    }
}

#[cfg(test)]
#[path = "doppler_tests.rs"]
mod doppler_tests;

#[cfg(test)]
#[path = "doppler_mix_tests.rs"]
mod doppler_mix_tests;
