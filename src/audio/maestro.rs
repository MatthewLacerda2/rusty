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
//! `RodioBackend` after construction. A play action always updates the deterministic
//! introspection log; only the *sound* is a backend side effect. The maestro never
//! reads a wall clock or unseeded RNG — voice ids are a monotone counter and the
//! event `tick` is supplied by the caller — so nothing here threatens replay.

use std::collections::HashMap;

use super::backend::{AudioBackend, NullBackend, PlayParams, VoiceId};
use super::introspection::{
    AudioEvent, AudioEventKind, AudioEventLog, VoiceInfo, DEFAULT_EVENT_CAP,
};
use super::spatial::{self, Listener};
use crate::components::AudioSourceComponent;

/// A live entity voice: the backend handle plus the per-source (pre-master) volume,
/// kept so a master-volume change can re-fold every voice.
struct LiveVoice {
    voice: VoiceId,
    source_volume: f32,
}

/// The audio engine singleton. See the module docs.
pub struct AudioMaestro {
    backend: Box<dyn AudioBackend>,
    /// Linear master gain in `[0, 1]`, multiplied into every voice.
    master_volume: f32,
    /// Live entity voices, keyed by owning entity id (one voice per source).
    entity_voices: HashMap<u32, LiveVoice>,
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
        Self {
            backend,
            master_volume: 1.0,
            entity_voices: HashMap::new(),
            log: AudioEventLog::new(DEFAULT_EVENT_CAP),
            next_voice: 1,
        }
    }

    /// Swap in a different backend (the windowed app injects the real `RodioBackend`
    /// after `GameWorld::new`). Any voices live on the old backend are forgotten.
    pub fn set_backend(&mut self, backend: Box<dyn AudioBackend>) {
        self.backend = backend;
        self.entity_voices.clear();
    }

    /// The current master volume.
    pub fn master_volume(&self) -> f32 {
        self.master_volume
    }

    /// Set the master volume (clamped to `[0, 1]`), re-folding every live voice.
    pub fn set_master_volume(&mut self, master: f32) {
        self.master_volume = master.clamp(0.0, 1.0);
        let folded: Vec<(VoiceId, f32)> = self
            .entity_voices
            .values()
            .map(|v| (v.voice, v.source_volume * self.master_volume))
            .collect();
        self.backend.set_master_volume(self.master_volume, &folded);
    }

    /// Mint the next backend voice id (deterministic monotone counter).
    fn mint(&mut self) -> VoiceId {
        let id = VoiceId(self.next_voice);
        self.next_voice += 1;
        id
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
        let voice = self.mint();
        let started = self.backend.play(
            voice,
            &PlayParams {
                clip: source.clip.clone(),
                volume: source_volume * self.master_volume,
                looping: source.looping,
            },
        );
        if started {
            self.entity_voices.insert(
                id,
                LiveVoice {
                    voice,
                    source_volume,
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

    /// Retune entity `id`'s live voice volume (pre-master); persisted so a later
    /// master change keeps the right ratio. No-op if the entity has no live voice.
    pub fn set_source_volume(&mut self, id: u32, volume: f32) {
        let master = self.master_volume;
        if let Some(live) = self.entity_voices.get_mut(&id) {
            live.source_volume = volume.max(0.0);
            self.backend
                .set_volume(live.voice, live.source_volume * master);
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
    pub fn play_at(
        &mut self,
        clip: &str,
        position: [f32; 3],
        volume: f32,
        source_entity: u32,
        tick: u64,
    ) -> bool {
        let volume = volume.max(0.0);
        let voice = self.mint();
        let started = self.backend.play(
            voice,
            &PlayParams {
                clip: clip.to_string(),
                volume: volume * self.master_volume,
                looping: false,
            },
        );
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

    /// Build a [`VoiceInfo`] for entity `id`'s source — the per-source roster row the
    /// agent reads, with the live playing flag folded in and the spatial `(gain, pan)`
    /// resolved against `listener` at the source's world `position` (#213). The caller
    /// supplies the component, position and listener (the maestro doesn't hold the
    /// scene or the active camera).
    pub fn voice_info(
        &self,
        id: u32,
        source: &AudioSourceComponent,
        position: glam::Vec3,
        listener: &Listener,
    ) -> VoiceInfo {
        let spatial = spatial::resolve(
            listener,
            position,
            source.volume,
            source.spatial_blend,
            source.initial_distance,
            source.final_distance,
        );
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
}

#[cfg(test)]
#[path = "maestro_tests.rs"]
mod maestro_tests;
