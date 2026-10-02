//! src/audio/device/backend.rs — the real audio device: kira's mixer (#212, #465).
//!
//! The platform-layer [`AudioBackend`] backed by [kira](https://docs.rs/kira): a
//! cpal device, a mixer of tracks with effects, and tweened parameters. Opened by
//! the windowed shell (`shell/boot.rs`) and injected into the `AudioMaestro` after
//! `GameWorld::new`, so the device only ever exists in the windowed process — never
//! in the harness, which keeps the `NullBackend`.
//!
//! The graph: each voice is a [`ClipSound`] playing into its mixer group's track
//! (`groups.rs`); group tracks nest as the group tree does and send to one reverb
//! bus; everything sums into kira's main track, whose [`OutputStage`] shapes the
//! total for the speaker mode (#546). The maestro hands over a [`VoiceMix`] per
//! voice each frame (#412) through the voice's shared [`VoiceControl`], and a
//! [`GroupMix`] per group whenever the mixer moves. Decoding is path-cached
//! ([`ClipCache`]), so the hundredth footstep costs no disk or decode.
//!
//! Generic over kira's `Backend` so tests render the whole graph headlessly
//! (`capture.rs`); the shell uses kira's cpal backend.

use std::collections::HashMap;
use std::sync::Arc;

use kira::backend::{Backend, DefaultBackend};
use kira::track::MainTrackBuilder;
use kira::{AudioManager, AudioManagerSettings, Capacities};

use super::decode::ClipCache;
use super::groups::{GroupTracks, VOICES_PER_GROUP};
use super::output::OutputStage;
use super::voice::{ClipSound, ClipSoundData, VoiceControl};
use crate::audio::backend::{AudioBackend, PlayParams, VoiceId, VoiceMix};
use crate::audio::mixer::{GroupId, GroupMix};
use crate::audio::SpeakerMode;

/// kira-backed mixer. Owns the `AudioManager`, which holds the device open for the
/// life of the backend — dropping it silences everything.
pub struct KiraBackend<B: Backend = DefaultBackend> {
    manager: AudioManager<B>,
    groups: GroupTracks,
    output: OutputStage,
    voices: HashMap<VoiceId, Arc<VoiceControl>>,
    cache: ClipCache,
}

impl KiraBackend {
    /// Open the default output device. Returns `None` when no device is available
    /// (a headless box, a CI runner) so the caller can fall back to the
    /// `NullBackend` instead of failing the whole app.
    pub fn open() -> Option<Self> {
        Self::with_backend(Default::default())
    }
}

impl<B: Backend> KiraBackend<B> {
    /// A mixer over kira backend `B` built with `settings`; `None` if it won't start.
    pub fn with_backend(settings: B::Settings) -> Option<Self> {
        let mut main = MainTrackBuilder::new().sound_capacity(VOICES_PER_GROUP);
        let output = OutputStage::build(&mut main, SpeakerMode::default());
        let mut manager = AudioManager::<B>::new(AudioManagerSettings {
            capacities: Capacities::default(),
            main_track_builder: main,
            internal_buffer_size: 128,
            backend_settings: settings,
        })
        .ok()?;
        Some(Self {
            groups: GroupTracks::new(&mut manager)?,
            manager,
            output,
            voices: HashMap::new(),
            cache: ClipCache::new(),
        })
    }

    /// kira's backend, for a test to pump.
    #[cfg(test)]
    pub fn device(&mut self) -> &mut B {
        self.manager.backend_mut()
    }

    /// The clip cache, for a test to seed with in-memory clips.
    #[cfg(test)]
    pub fn cache(&mut self) -> &mut ClipCache {
        &mut self.cache
    }
}

impl<B: Backend> AudioBackend for KiraBackend<B> {
    fn play(&mut self, id: VoiceId, params: &PlayParams) -> bool {
        let Some(clip) = self.cache.get_or_decode(&params.clip) else {
            return false;
        };
        let Some(track) = self.groups.track(params.group) else {
            return false;
        };
        // Mix before starting so the first samples already play at the resolved
        // state (a paused voice starts paused).
        let control = VoiceControl::new(&params.mix);
        let sound = ClipSound::new(clip, params.looping, Arc::clone(&control));
        if track.play(ClipSoundData(sound)).is_err() {
            return false;
        }
        // A previous voice on the same id (re-`Play`) is replaced and stopped.
        if let Some(old) = self.voices.insert(id, control) {
            old.stop();
        }
        true
    }

    fn stop(&mut self, id: VoiceId) {
        if let Some(voice) = self.voices.remove(&id) {
            voice.stop();
        }
    }

    fn set_mix(&mut self, id: VoiceId, mix: &VoiceMix) {
        if let Some(voice) = self.voices.get(&id) {
            voice.set(mix);
        }
    }

    fn is_live(&self, id: VoiceId) -> bool {
        self.voices.get(&id).is_some_and(|v| v.is_live())
    }

    fn stop_all(&mut self) {
        for (_, voice) in self.voices.drain() {
            voice.stop();
        }
    }

    fn set_speaker_mode(&mut self, mode: SpeakerMode) {
        self.output.set(mode);
    }

    fn add_group(&mut self, id: GroupId, parent: Option<GroupId>) {
        self.groups.add(&mut self.manager, id, parent);
    }

    fn set_group(&mut self, id: GroupId, mix: &GroupMix) {
        self.groups.set(id, mix);
    }
}

#[cfg(test)]
#[path = "backend_tests.rs"]
pub(crate) mod backend_tests;
