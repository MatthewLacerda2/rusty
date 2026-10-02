//! src/audio/maestro.rs — AudioMaestro: the audio engine singleton (Resource, #212).
//!
//! On the same shelf as `Time`, `Input`, the active camera and the renderer: one per
//! World. It owns the audio device/mixer (a `Box<dyn AudioBackend>`), enforces a
//! single **master volume**, tracks which entity `AudioSource`s are live, mints
//! fire-and-forget one-shot voices, and is the home for the agent-facing
//! introspection (the roster + the event log).
//!
//! It runs in the **platform layer**, not the sim: by default it carries a
//! [`NullBackend`] (the harness path), and the windowed app swaps in the real
//! `KiraBackend` after construction. It also owns the [`Mixer`] (#465), whose groups
//! every voice routes into (`mixer/maestro.rs`). A play action always updates the
//! deterministic introspection log; only the *sound* is a backend side effect. Voice
//! ids are a monotone counter and the event `tick` is supplied by the caller — no
//! wall clock or unseeded RNG, so nothing here threatens replay.

use std::collections::{BTreeMap, HashMap};

use glam::Vec3;

use super::backend::{AudioBackend, NullBackend, PlayParams, VoiceId, VoiceMix};
use super::introspection::{AudioEvent, AudioEventKind, AudioEventLog, DEFAULT_EVENT_CAP};
use super::mix::{self, MixEnv};
use super::mixer::{GroupId, Mixer};
use super::occlusion::Occlusion;
use super::speaker::SpeakerMode;
use crate::components::AudioSourceComponent;

/// A live entity voice: the backend handle, the source settings and position it was
/// last resolved from, its per-source (pre-master) volume, and the pre-master mix
/// last handed to the backend — so a master change can re-fold it and the per-frame
/// mix only sends what changed.
pub(super) struct LiveVoice {
    pub(super) voice: VoiceId,
    pub(super) source: AudioSourceComponent,
    pub(super) source_volume: f32,
    pub(super) position: Vec3,
    pub(super) mix: VoiceMix,
    pub(super) group: GroupId,
}

/// A fire-and-forget `PlayAt` voice still sounding: tracked only so the per-frame
/// mix can keep it spatialized and time-scaled until the backend reports it done.
pub(super) struct OneShot {
    pub(super) source: AudioSourceComponent,
    pub(super) volume: f32,
    pub(super) position: Vec3,
    pub(super) mix: VoiceMix,
    pub(super) group: GroupId,
}

/// The audio engine singleton. See the module docs.
pub struct AudioMaestro {
    pub(super) backend: Box<dyn AudioBackend>,
    /// Linear master gain in `[0, 1]`, multiplied into every voice.
    pub(super) master_volume: f32,
    /// The output profile every voice is shaped for (#546); output only — the sim
    /// never reads it.
    pub(super) speaker_mode: SpeakerMode,
    /// Live entity voices, keyed by owning entity id (one voice per source).
    pub(super) entity_voices: HashMap<u32, LiveVoice>,
    /// Live `PlayAt` one-shots, reaped once the backend reports them finished.
    pub(super) oneshots: BTreeMap<VoiceId, OneShot>,
    /// The listener + clock state of the last per-frame mix; a voice started between
    /// frames is resolved against it so its first samples are already mixed.
    pub(super) env: MixEnv,
    /// The mixer groups, snapshots and ducks every voice routes through (#465).
    pub(super) mixer: Mixer,
    /// Every voice's occlusion factor and the casts behind it (#467).
    pub(super) occlusion: Occlusion,
    /// The agent-facing log of play/stop/one-shot actions.
    log: AudioEventLog,
    /// Monotone id source for backend voices (deterministic — never a clock/RNG).
    next_voice: u64,
}

impl Default for AudioMaestro {
    fn default() -> Self {
        Self::with_backend(Box::new(NullBackend))
    }
}

impl AudioMaestro {
    /// Build a maestro over `backend`. `GameWorld::new` uses the `Default`
    /// (`NullBackend`); the windowed app calls [`AudioMaestro::set_backend`] with the
    /// real device afterwards.
    pub fn with_backend(backend: Box<dyn AudioBackend>) -> Self {
        let mut maestro = Self {
            backend,
            master_volume: 1.0,
            speaker_mode: SpeakerMode::default(),
            entity_voices: HashMap::new(),
            oneshots: BTreeMap::new(),
            env: MixEnv::default(),
            mixer: Mixer::default(),
            occlusion: Occlusion::default(),
            log: AudioEventLog::new(DEFAULT_EVENT_CAP),
            next_voice: 1,
        };
        maestro.sync_groups();
        maestro
    }

    /// Swap in a different backend (the windowed app injects the real `KiraBackend`
    /// after `GameWorld::new`). Any voices live on the old backend are forgotten;
    /// the new one takes the current speaker mode and mixer groups.
    pub fn set_backend(&mut self, mut backend: Box<dyn AudioBackend>) {
        backend.set_speaker_mode(self.speaker_mode);
        self.backend = backend;
        self.entity_voices.clear();
        self.oneshots.clear();
        self.sync_groups();
    }

    /// The current master volume.
    pub fn master_volume(&self) -> f32 {
        self.master_volume
    }

    /// Set the master volume (clamped to `[0, 1]`), re-folding every live voice.
    pub fn set_master_volume(&mut self, master: f32) {
        self.master_volume = master.clamp(0.0, 1.0);
        self.refold_master();
    }

    /// The current speaker mode (#546).
    pub fn speaker_mode(&self) -> SpeakerMode {
        self.speaker_mode
    }

    /// Switch the speaker mode: the output stage is retuned and every live voice
    /// re-sent with the new per-voice shaping.
    pub fn set_speaker_mode(&mut self, mode: SpeakerMode) {
        self.speaker_mode = mode;
        self.backend.set_speaker_mode(mode);
        self.refold_master();
    }

    /// `mix` as the backend receives it: master volume + speaker-mode shaping.
    pub(super) fn output(&self, mix: VoiceMix) -> VoiceMix {
        mix.for_output(self.master_volume, self.speaker_mode)
    }

    /// Start a voice on the backend under the next id (a deterministic monotone
    /// counter), returning the id and whether the backend accepted it.
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

    /// Start (or restart) entity `id`'s `AudioSource`. The `position` and `tick` are
    /// supplied by the caller (its transform + the play frame). Returns whether the
    /// backend accepted the voice; the action is logged regardless.
    pub fn play_source(
        &mut self,
        id: u32,
        source: &AudioSourceComponent,
        position: [f32; 3],
        tick: u64,
    ) -> bool {
        // Replace any existing voice on this entity first.
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
                    mix,
                    group,
                },
            );
        }
        self.log.push(AudioEvent {
            kind: AudioEventKind::Play,
            clip: source.clip.clone(),
            entity: id,
            position,
            volume: source_volume,
            tick,
        });
        started
    }

    /// Stop entity `id`'s voice if live. `log_stop` controls whether a `Stop` event
    /// is recorded (suppressed when stopping only to immediately replace the voice).
    pub fn stop_source(&mut self, id: u32, position: [f32; 3], tick: u64, log_stop: bool) {
        if let Some(live) = self.entity_voices.remove(&id) {
            self.backend.stop(live.voice);
        }
        if log_stop {
            self.log.push(AudioEvent {
                kind: AudioEventKind::Stop,
                clip: String::new(),
                entity: id,
                position,
                volume: 0.0,
                tick,
            });
        }
    }

    /// Whether entity `id` currently has a live voice.
    pub fn is_source_playing(&self, id: u32) -> bool {
        self.entity_voices.contains_key(&id)
    }

    /// Fire a one-shot at a world position — `Audio.PlayAt`. It owns no entity voice
    /// (it is fire-and-forget), so it is *only* recorded in the event log; that log
    /// entry is the one-shot's sole trace. `source_entity` attributes it to an
    /// emitter (or `0`). Returns whether the backend accepted it.
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
            kind: AudioEventKind::PlayAt,
            clip: clip.to_string(),
            entity: source_entity,
            position,
            volume,
            tick,
        });
        started
    }

    /// Stop every live entity voice (e.g. on leaving Play). One-shots already in
    /// flight are stopped too via the backend's `stop_all`.
    pub fn stop_all(&mut self) {
        self.entity_voices.clear();
        self.oneshots.clear();
        self.backend.stop_all();
    }

    /// The agent-facing event log (play / stop / one-shot), oldest first.
    pub fn events(&self) -> &[AudioEvent] {
        self.log.events()
    }

    /// Clear the event log (e.g. on entering Play for a clean record).
    pub fn clear_log(&mut self) {
        self.log.clear();
    }
}

#[cfg(test)]
#[path = "maestro_tests.rs"]
mod maestro_tests;

#[cfg(test)]
#[path = "speaker_mix_tests.rs"]
mod speaker_mix_tests;
