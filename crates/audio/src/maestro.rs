use std::collections::{BTreeMap, BTreeSet};

use rusty_core::collections::Map;

use glam::Vec3;

use super::backend::{AudioBackend, NullBackend, PlayParams, VoiceId, VoiceMix};
use super::doppler::DopplerState;
use super::introspection::{AudioEvent, AudioEventKind, AudioEventLog, DEFAULT_EVENT_CAP};
use super::mix::{self, MixEnv};
use super::mixer::{GroupId, Mixer};
use super::occlusion::Occlusion;
use super::reverb::ReverbState;
use super::speaker::SpeakerMode;
use rusty_components::AudioSourceComponent;

pub(super) struct LiveVoice {
    pub(super) voice: VoiceId,
    pub(super) source: AudioSourceComponent,
    pub(super) source_volume: f32,
    pub(super) position: Vec3,
    pub(super) heard_at: Vec3,
    pub(super) mix: VoiceMix,
    pub(super) group: GroupId,
}

pub(super) struct OneShot {
    pub(super) source: AudioSourceComponent,
    pub(super) volume: f32,
    pub(super) position: Vec3,
    pub(super) mix: VoiceMix,
    pub(super) group: GroupId,
}

pub struct AudioMaestro {
    pub(super) backend: Box<dyn AudioBackend>,
    /// Linear master gain in `[0, 1]`, multiplied into every voice.
    pub(super) master_volume: f32,
    pub(super) speaker_mode: SpeakerMode,
    pub(super) entity_voices: Map<u32, LiveVoice>,
    pub(super) oneshots: BTreeMap<VoiceId, OneShot>,
    pub(super) env: MixEnv,
    pub(super) mixer: Mixer,
    pub(super) doppler: DopplerState,
    pub(super) occlusion: Occlusion,
    pub(super) reverb: ReverbState,
    pub(super) log: AudioEventLog,
    pub(super) enabled_sources: BTreeSet<u32>,
    /// Monotone id source for backend voices (deterministic — never a clock/RNG).
    next_voice: u64,
    pub(super) capture: Option<super::capture::Recorder>,
}

impl Default for AudioMaestro {
    fn default() -> Self {
        Self::with_backend(Box::new(NullBackend))
    }
}

impl AudioMaestro {
    pub fn with_backend(backend: Box<dyn AudioBackend>) -> Self {
        let mut maestro = Self {
            backend,
            master_volume: 1.0,
            speaker_mode: SpeakerMode::default(),
            entity_voices: Map::default(),
            oneshots: BTreeMap::new(),
            env: MixEnv::default(),
            mixer: Mixer::default(),
            doppler: DopplerState::default(),
            occlusion: Occlusion::default(),
            reverb: ReverbState::DRY,
            log: AudioEventLog::new(DEFAULT_EVENT_CAP),
            enabled_sources: BTreeSet::new(),
            next_voice: 1,
            capture: None,
        };
        maestro.sync_groups();
        maestro
    }

    pub fn set_backend(&mut self, mut backend: Box<dyn AudioBackend>) {
        backend.set_speaker_mode(self.speaker_mode);
        self.backend = backend;
        self.entity_voices.clear();
        self.oneshots.clear();
        self.sync_groups();
        self.backend.set_reverb(&self.reverb.params);
    }

    pub fn master_volume(&self) -> f32 {
        self.master_volume
    }

    /// Set the master volume (clamped to `[0, 1]`), re-folding every live voice.
    pub fn set_master_volume(&mut self, master: f32) {
        self.master_volume = master.clamp(0.0, 1.0);
        self.refold_master();
    }

    pub fn speaker_mode(&self) -> SpeakerMode {
        self.speaker_mode
    }

    pub fn set_speaker_mode(&mut self, mode: SpeakerMode) {
        self.speaker_mode = mode;
        self.backend.set_speaker_mode(mode);
        self.refold_master();
    }

    pub(super) fn output(&self, mix: VoiceMix) -> VoiceMix {
        mix.for_output(self.master_volume, self.speaker_mode)
    }

    fn start(&mut self, clip: &str, looping: bool, mix: VoiceMix, to: GroupId) -> (VoiceId, bool) {
        let voice = VoiceId(self.next_voice);
        self.next_voice += 1;
        let params = PlayParams {
            clip: clip.to_string(),
            looping,
            mix: self.output(mix),
            group: to,
        };
        (voice, self.backend.play(voice, &params))
    }

    pub fn play_source(
        &mut self,
        id: u32,
        source: &AudioSourceComponent,
        position: [f32; 3],
        tick: u64,
    ) -> bool {
        self.stop_source(id, position, tick, /*log_stop=*/ false);
        let source_volume = source.volume.max(0.0);
        let at = Vec3::from(position);
        let (occluded, mix) = self.resolve_start(source, Some(id), source_volume, at);
        let group = self.route(&source.output_group);
        let (voice, started) = self.start(&source.clip, source.looping, mix, group);
        if started {
            self.track_occlusion(voice, occluded);
            self.entity_voices.insert(
                id,
                LiveVoice {
                    voice,
                    source: source.clone(),
                    source_volume,
                    position: at,
                    heard_at: at,
                    mix,
                    group,
                },
            );
        }
        self.log.push(AudioEvent {
            seq: 0,
            kind: AudioEventKind::Play,
            clip: source.clip.clone(),
            entity: id,
            position,
            volume: source_volume,
            tick,
        });
        started
    }

    pub fn stop_source(&mut self, id: u32, position: [f32; 3], tick: u64, log_stop: bool) {
        if let Some(live) = self.entity_voices.remove(&id) {
            self.backend.stop(live.voice);
        }
        if log_stop {
            self.log.push(AudioEvent {
                seq: 0,
                kind: AudioEventKind::Stop,
                clip: String::new(),
                entity: id,
                position,
                volume: 0.0,
                tick,
            });
        }
    }

    pub fn is_source_playing(&self, id: u32) -> bool {
        self.entity_voices.contains_key(&id)
    }

    pub fn play_at(&mut self, shot: &mix::Shot, source_entity: u32, tick: u64) -> bool {
        let mix::Shot {
            clip,
            position,
            volume,
            rolloff,
            group,
        } = *shot;
        let volume = volume.max(0.0);
        let source = mix::oneshot_source(clip, rolloff);
        let at = Vec3::from(position);
        let (occluded, mix) = self.resolve_start(&source, None, volume, at);
        let group = self.route(group);
        let (voice, started) = self.start(clip, false, mix, group);
        if started {
            self.track_occlusion(voice, occluded);
            let position = at;
            self.oneshots.insert(
                voice,
                OneShot {
                    source,
                    volume,
                    position,
                    mix,
                    group,
                },
            );
        }
        self.log.push(AudioEvent {
            seq: 0,
            kind: AudioEventKind::PlayAt,
            clip: clip.to_string(),
            entity: source_entity,
            position,
            volume,
            tick,
        });
        started
    }

    pub fn stop_all(&mut self) {
        self.entity_voices.clear();
        self.oneshots.clear();
        self.enabled_sources.clear();
        self.doppler.forget();
        self.backend.stop_all();
    }
}

#[cfg(test)]
#[path = "maestro_tests.rs"]
mod maestro_tests;

#[cfg(test)]
#[path = "speaker_mix_tests.rs"]
mod speaker_mix_tests;
