//! crates/audio/src/doppler.rs — doppler shift on 3D sources (#1749).
//!
//! Unity's *Doppler Level*: a source closing on the listener plays faster (higher),
//! one moving away slower. The rate a voice plays at is
//!
//! ```text
//! rate = (c − v_listener·n) / (c − v_source·n)
//! ```
//!
//! with `n` the unit vector from the source to the listener and `c` the speed of
//! sound (a project setting, 343 m/s by default: `project.rusty`'s `"audio"`). Each
//! velocity is scaled by the source's effective level, `doppler_level ×
//! spatial_blend`, so a 2D source (`spatial_blend = 0`) never shifts and a level
//! above 1 exaggerates it. The rate is clamped to [`RATE_MIN`]`..=`[`RATE_MAX`] and
//! multiplies the time-scale rate; the device glides it over a few ms
//! (`device/voice.rs`), so a step in velocity is never heard as a click.
//!
//! **Where the velocities come from.** Every body's velocity is its world position's
//! motion across the tick, measured where the mix is pushed (`AudioMaestro::apply_mix`):
//! the same for a Transform moved by a script, a Rigidbody, a CharacterController
//! and a source parented under any of them, since all of them end the tick at a
//! world position. It is divided by the tick's scaled sim time, so slow motion keeps
//! the shift a body's real speed gives, and the time scale bends the pitch on top.
//! A tick with no sim time (a pause, `timeScale = 0`) keeps a time-scaled voice's
//! rate as it was; an unscaled voice, which plays at normal rate through a pause,
//! returns to 1.
//!
//! **Jumps.** A scene load and Play/Stop forget the listener's motion, so the next
//! push hears it still. A body whose move in one tick is faster than
//! [`TELEPORT_SPEED`] (far above any projectile) is taken as a **teleport** and
//! counts as still for that tick. A shorter jump is not told apart from motion: it
//! reads as velocity for that one tick (3 m toward the listener at 60 Hz is
//! 180 m/s), clamped, and the device's glide smooths it.
//!
//! Output only: the sim never reads a rate, so determinism is untouched.

use glam::Vec3;

use super::backend::VoiceMix;
use super::maestro::AudioMaestro;
use super::mix::MixEnv;
use rusty_components::AudioSourceComponent;

pub use rusty_core::settings::audio::SPEED_OF_SOUND;
/// The slowest a doppler shift plays a voice (an octave down).
pub const RATE_MIN: f32 = 0.5;
/// The fastest a doppler shift plays a voice (an octave up).
pub const RATE_MAX: f32 = 2.0;
/// The speed, in m/s, past which one tick's move is a teleport and counts as still.
/// Not the speed of sound: a body faster than sound plays at the clamp, and a slow
/// speed of sound (`Audio.SetSpeedOfSound`) must not silence real motion.
pub const TELEPORT_SPEED: f32 = 1000.0;

/// The motion the next push measures against: the listener's position at the last
/// push (`None` once forgotten) and the speed of sound in force.
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
    /// The listener as this push hears it, its velocity measured from the last
    /// push's position (still when that was forgotten), which it then remembers.
    pub fn listen(&mut self, env: &MixEnv) -> Body {
        let now = env.listener.position;
        let was = self.listener_was.replace(now).unwrap_or(now);
        Body {
            position: now,
            velocity: velocity(was, now, env.dt),
        }
    }

    /// Forget the listener's motion: the next push measures it as still (Play,
    /// Stop, a scene load).
    pub fn forget(&mut self) {
        self.listener_was = None;
    }
}

impl AudioMaestro {
    /// The speed of sound doppler resolves with, in m/s.
    pub fn speed_of_sound(&self) -> f32 {
        self.doppler.speed_of_sound
    }

    /// Set the speed of sound (`Audio.SetSpeedOfSound`, the project's on entering
    /// Play and Stop). A value that is not finite and positive is refused: it keeps
    /// the one in force and returns `false`.
    pub fn set_speed_of_sound(&mut self, c: f32) -> bool {
        let ok = c.is_finite() && c > 0.0;
        if ok {
            self.doppler.speed_of_sound = c;
        }
        ok
    }

    /// Forget the listener's motion, so the next push hears it still (a scene load).
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

/// The doppler rate a voice of `source` plays at this push, having moved
/// `was → now`. A push with no sim time keeps the rate `held` last played at for a
/// time-scaled voice, and plays an unscaled one (which a pause doesn't stop) at 1.
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

/// The velocity a body moving `was → now` over `dt` seconds had, or zero when its
/// move is faster than [`TELEPORT_SPEED`] (a teleport), or nothing finite.
pub fn velocity(was: Vec3, now: Vec3, dt: f32) -> Vec3 {
    let v = (now - was) / dt;
    if dt > 0.0 && v.is_finite() && v.length() < TELEPORT_SPEED {
        v
    } else {
        Vec3::ZERO
    }
}

/// One body in a doppler resolve: where it is and how fast it moves.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Body {
    pub position: Vec3,
    pub velocity: Vec3,
}

/// The doppler rate a `source` plays at for `listener`, at speed of sound `c`, with
/// both velocities scaled by `level` (`doppler_level × spatial_blend`). `1.0` at
/// level 0, for a co-located pair and for anything not finite.
pub fn rate(listener: Body, source: Body, c: f32, level: f32) -> f32 {
    let Some(n) = (listener.position - source.position).try_normalize() else {
        return 1.0;
    };
    if level <= 0.0 || c <= 0.0 {
        return 1.0;
    }
    // The speed each body closes along `n`, source → listener.
    let along = |v: Vec3| (v * level).dot(n);
    let (heard, sent) = (along(listener.velocity), along(source.velocity));
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
