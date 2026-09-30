//! src/audio/mix.rs — the per-frame mix: what each live voice sounds like now (#412).
//!
//! `spatial::resolve` computes a source's `(gain, pan)`; this module turns that plus
//! `Time.timeScale` / `Time.Pause` into the [`VoiceMix`] the backend applies, and
//! [`AudioMaestro::apply_mix`] re-resolves every live voice once a frame against the
//! current listener. [`resolve_voice`] is the **one** call behind both the applied
//! mix and `Audio.GetSpatial` (`AudioMaestro::voice_info`), so the headless
//! read-back and the device cannot drift apart.
//!
//! Time scaling: a voice with `is_time_scaled` plays at rate `Time.timeScale` and is
//! held paused while `timeScale == 0` or the loop-level `Time.Pause` is on; it
//! resumes where it left off. Unscaled voices (music, UI) ignore both.
//!
//! One-shots (`Audio.PlayAt`) are diegetic: fully spatial (`spatial_blend = 1`) and
//! time-scaled, with the `AudioSource` default rolloff band unless the caller passes
//! a [`Rolloff`] (#575 — a gunshot that must carry past 16 m).
//!
//! Pure and clock-free — the caller supplies the listener and the time state.

use glam::{Quat, Vec3};

use super::backend::VoiceMix;
use super::introspection::{SpatialResult, VoiceInfo};
use super::maestro::AudioMaestro;
use super::spatial::{self, Listener};
use crate::components::AudioSourceComponent;

/// What the mix depends on besides the source: the listener and the clock state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MixEnv {
    pub listener: Listener,
    /// `Time.timeScale`.
    pub time_scale: f32,
    /// The loop-level `Time.Pause` flag.
    pub paused: bool,
}

impl Default for MixEnv {
    /// A listener at the origin facing -Z, realtime, unpaused — used until the
    /// first frame's [`AudioMaestro::apply_mix`] supplies the real one.
    fn default() -> Self {
        Self {
            listener: Listener::from_transform(Vec3::ZERO, Quat::IDENTITY),
            time_scale: 1.0,
            paused: false,
        }
    }
}

/// Resolve one voice's pre-master mix: `source`'s spatial + time-scale fields at
/// world `position`, with `volume` as its per-source gain.
pub fn resolve_voice(
    env: &MixEnv,
    source: &AudioSourceComponent,
    volume: f32,
    position: Vec3,
) -> VoiceMix {
    let SpatialResult { gain, pan } = spatial::resolve(
        &env.listener,
        position,
        volume,
        source.spatial_blend,
        source.initial_distance,
        source.final_distance,
    );
    let scaled = source.is_time_scaled;
    VoiceMix {
        gain,
        pan,
        spatial_blend: source.spatial_blend.clamp(0.0, 1.0),
        speed: if scaled { env.time_scale } else { 1.0 },
        paused: scaled && (env.paused || env.time_scale <= 0.0),
    }
}

/// A one-shot's linear rolloff band: full volume out to `min_distance`, silent
/// at/beyond `max_distance` — the `AudioSource` `initial_distance`/`final_distance`
/// pair, chosen per shot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rolloff {
    pub min_distance: f32,
    pub max_distance: f32,
}

impl Default for Rolloff {
    /// The `AudioSource` default band (1 → 16 m), Unity's `PlayClipAtPoint` reach.
    fn default() -> Self {
        let source = AudioSourceComponent::default();
        Self {
            min_distance: source.initial_distance,
            max_distance: source.final_distance,
        }
    }
}

/// One `Audio.PlayAt` call: what to play, where, how loud, and how far it carries.
#[derive(Clone, Copy, Debug)]
pub struct Shot<'a> {
    pub clip: &'a str,
    pub position: [f32; 3],
    /// Per-shot linear gain (pre-master).
    pub volume: f32,
    pub rolloff: Rolloff,
}

impl<'a> Shot<'a> {
    /// `clip` at `position`, full volume, on the default band.
    pub fn new(clip: &'a str, position: [f32; 3]) -> Self {
        Self {
            clip,
            position,
            volume: 1.0,
            rolloff: Rolloff::default(),
        }
    }
}

/// The emitter settings an `Audio.PlayAt` one-shot plays with.
pub fn oneshot_source(clip: &str, rolloff: Rolloff) -> AudioSourceComponent {
    AudioSourceComponent {
        clip: clip.to_string(),
        spatial_blend: 1.0,
        is_time_scaled: true,
        initial_distance: rolloff.min_distance,
        final_distance: rolloff.max_distance,
        ..Default::default()
    }
}

impl AudioMaestro {
    /// Re-resolve every live voice against `env` and hand changed mixes to the
    /// backend — the per-frame step the windowed shell runs after the sim advances.
    /// `lookup` gives an entity's current `AudioSource` + world position; an entity
    /// that no longer has one keeps its last settings. Finished one-shots are reaped.
    pub fn apply_mix(
        &mut self,
        env: &MixEnv,
        lookup: impl Fn(u32) -> Option<(AudioSourceComponent, Vec3)>,
    ) {
        self.env = *env;
        let master = self.master_volume;
        for (&id, live) in self.entity_voices.iter_mut() {
            if let Some((source, position)) = lookup(id) {
                live.source = source;
                live.position = position;
            }
            let mix = resolve_voice(env, &live.source, live.source_volume, live.position);
            if mix != live.mix {
                live.mix = mix;
                self.backend.set_mix(live.voice, &mix.with_master(master));
            }
        }
        let backend = &mut self.backend;
        self.oneshots.retain(|&voice, shot| {
            if !backend.is_live(voice) {
                backend.stop(voice);
                return false;
            }
            let mix = resolve_voice(env, &shot.source, shot.volume, shot.position);
            if mix != shot.mix {
                shot.mix = mix;
                backend.set_mix(voice, &mix.with_master(master));
            }
            true
        });
    }

    /// Build a [`VoiceInfo`] for entity `id`'s source — the per-source roster row the
    /// agent reads, with the live playing flag folded in and the spatial `(gain, pan)`
    /// resolved against `listener` at the source's world `position` (#213) — by the same
    /// [`resolve_voice`] call the per-frame mix applies (#412), with the live
    /// voice's volume when it is playing. The caller supplies the component, position
    /// and listener (the maestro doesn't hold the scene or the active camera).
    pub fn voice_info(
        &self,
        id: u32,
        source: &AudioSourceComponent,
        position: glam::Vec3,
        listener: &Listener,
    ) -> VoiceInfo {
        let volume = self
            .entity_voices
            .get(&id)
            .map_or(source.volume.max(0.0), |live| live.source_volume);
        let env = MixEnv {
            listener: *listener,
            ..self.env
        };
        let mix = resolve_voice(&env, source, volume, position);
        let spatial = SpatialResult {
            gain: mix.gain,
            pan: mix.pan,
        };
        VoiceInfo {
            entity: id,
            clip: source.clip.clone(),
            volume: source.volume,
            looping: source.looping,
            is_time_scaled: source.is_time_scaled,
            playing: self.is_source_playing(id),
            spatial,
        }
    }

    /// Re-send every live voice's current mix folded with the current master.
    pub(super) fn refold_master(&mut self) {
        let master = self.master_volume;
        for live in self.entity_voices.values() {
            self.backend
                .set_mix(live.voice, &live.mix.with_master(master));
        }
        for (&voice, shot) in &self.oneshots {
            self.backend.set_mix(voice, &shot.mix.with_master(master));
        }
    }

    /// The pre-master mix last applied to entity `id`'s voice, if it is playing.
    pub fn voice_mix(&self, id: u32) -> Option<VoiceMix> {
        self.entity_voices.get(&id).map(|live| live.mix)
    }

    /// How many `PlayAt` one-shots are still sounding.
    pub fn live_oneshots(&self) -> usize {
        self.oneshots.len()
    }
}

#[cfg(test)]
#[path = "mix_tests.rs"]
mod mix_tests;

#[cfg(test)]
#[path = "oneshot_tests.rs"]
mod oneshot_tests;
