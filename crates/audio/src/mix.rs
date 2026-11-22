use glam::{Quat, Vec3};

use super::backend::VoiceMix;
use super::doppler;
use super::introspection::{SpatialResult, VoiceInfo};
use super::maestro::AudioMaestro;
use super::occlusion;
use super::spatial::{self, Listener};
use rusty_components::AudioSourceComponent;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MixEnv {
    pub listener: Listener,
    pub time_scale: f32,
    pub paused: bool,
    pub dt: f32,
}

impl Default for MixEnv {
    /// A listener at the origin facing -Z, realtime, unpaused — used until the
    /// first frame's [`AudioMaestro::apply_mix`] supplies the real one.
    fn default() -> Self {
        Self {
            listener: Listener::from_transform(Vec3::ZERO, Quat::IDENTITY),
            time_scale: 1.0,
            paused: false,
            dt: 0.0,
        }
    }
}

pub fn resolve_voice(
    env: &MixEnv,
    source: &AudioSourceComponent,
    volume: f32,
    position: Vec3,
    occlusion: f32,
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
    let (muffle, low_pass) = occlusion::effect(occlusion);
    VoiceMix {
        gain: gain * muffle,
        pan,
        spatial_blend: source.spatial_blend.clamp(0.0, 1.0),
        speed: if scaled { env.time_scale } else { 1.0 },
        doppler: 1.0,
        paused: scaled && (env.paused || env.time_scale <= 0.0),
        low_pass,
    }
}

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

#[derive(Clone, Copy, Debug)]
pub struct Shot<'a> {
    pub clip: &'a str,
    pub position: [f32; 3],
    pub volume: f32,
    pub rolloff: Rolloff,
    pub group: &'a str,
}

impl<'a> Shot<'a> {
    pub fn new(clip: &'a str, position: [f32; 3]) -> Self {
        Self {
            clip,
            position,
            volume: 1.0,
            rolloff: Rolloff::default(),
            group: "",
        }
    }
}

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
    pub fn apply_mix(
        &mut self,
        env: &MixEnv,
        lookup: impl Fn(u32) -> Option<(AudioSourceComponent, Vec3)>,
    ) {
        self.env = *env;
        let (master, mode) = (self.master_volume, self.speaker_mode);
        let gone: Vec<u32> = self
            .entity_voices
            .keys()
            .copied()
            .filter(|&id| lookup(id).is_none())
            .collect();
        for id in gone {
            if let Some(live) = self.entity_voices.remove(&id) {
                self.backend.stop(live.voice);
            }
        }
        let (occlusion, ear) = (&self.occlusion, self.doppler.listen(env));
        let c = self.doppler.speed_of_sound;
        for (&id, live) in self.entity_voices.iter_mut() {
            if let Some((source, position)) = lookup(id) {
                live.source = source;
                live.position = position;
            }
            let (at, occluded) = (live.position, occlusion.applied(live.voice));
            let was = std::mem::replace(&mut live.heard_at, at);
            let rate = doppler::source_rate(env, ear, (was, at), &live.source, c, live.mix);
            let mix = resolve_voice(env, &live.source, live.source_volume, at, occluded)
                .with_doppler(rate);
            if mix != live.mix {
                live.mix = mix;
                self.backend
                    .set_mix(live.voice, &mix.for_output(master, mode));
            }
        }
        let backend = &mut self.backend;
        self.oneshots.retain(|&voice, shot| {
            if !backend.is_live(voice) {
                backend.stop(voice);
                return false;
            }
            let (at, occluded) = (shot.position, occlusion.applied(voice));
            let rate = doppler::source_rate(env, ear, (at, at), &shot.source, c, shot.mix);
            let mix =
                resolve_voice(env, &shot.source, shot.volume, at, occluded).with_doppler(rate);
            if mix != shot.mix {
                shot.mix = mix;
                backend.set_mix(voice, &mix.for_output(master, mode));
            }
            true
        });
    }

    pub fn voice_info(
        &self,
        id: u32,
        source: &AudioSourceComponent,
        position: glam::Vec3,
        listener: &Listener,
    ) -> VoiceInfo {
        let live = self.entity_voices.get(&id);
        let volume = live.map_or(source.volume.max(0.0), |live| live.source_volume);
        let occluded = live.map_or(0.0, |live| self.occlusion.applied(live.voice));
        let env = MixEnv {
            listener: *listener,
            ..self.env
        };
        let mix = resolve_voice(&env, source, volume, position, occluded);
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
            occlusion: self.source_occlusion(id),
        }
    }

    pub fn set_source_volume(&mut self, id: u32, volume: f32) {
        let (master, mode) = (self.master_volume, self.speaker_mode);
        if let Some(live) = self.entity_voices.get_mut(&id) {
            live.source_volume = volume.max(0.0);
            let occluded = self.occlusion.applied(live.voice);
            let (source, at) = (&live.source, live.position);
            live.mix = resolve_voice(&self.env, source, live.source_volume, at, occluded)
                .with_doppler(live.mix.doppler);
            self.backend
                .set_mix(live.voice, &live.mix.for_output(master, mode));
        }
    }

    pub(super) fn refold_master(&mut self) {
        let (master, mode) = (self.master_volume, self.speaker_mode);
        for live in self.entity_voices.values() {
            self.backend
                .set_mix(live.voice, &live.mix.for_output(master, mode));
        }
        for (&voice, shot) in &self.oneshots {
            self.backend
                .set_mix(voice, &shot.mix.for_output(master, mode));
        }
    }

    #[doc(hidden)]
    pub fn voice_mix(&self, id: u32) -> Option<VoiceMix> {
        self.entity_voices.get(&id).map(|live| live.mix)
    }

    pub fn is_sounding(&self) -> bool {
        let mut voices = self.entity_voices.values();
        voices.any(|live| self.backend.is_live(live.voice))
            || !self.oneshots.is_empty()
            || self.capture.is_some()
    }

    #[cfg(test)]
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
