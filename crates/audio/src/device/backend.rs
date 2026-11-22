use rusty_core::collections::Map;
use std::sync::Arc;

use kira::backend::{Backend, DefaultBackend};
use kira::track::MainTrackBuilder;
use kira::{AudioManager, AudioManagerSettings, Capacities};

use super::capture::Clocked;
use super::decode::ClipCache;
use super::groups::{GroupTracks, MasterStrip, VOICES_PER_GROUP};
use super::output::OutputStage;
use super::tap::{Tap, TapEffect};
use super::voice::{ClipSound, ClipSoundData, VoiceControl};
use crate::backend::{AudioBackend, PlayParams, VoiceId, VoiceMix};
use crate::mixer::{GroupId, GroupMix};
use crate::SpeakerMode;
use rusty_components::ReverbParams;

pub struct KiraBackend<B: Backend = DefaultBackend> {
    manager: AudioManager<B>,
    groups: GroupTracks,
    output: OutputStage,
    voices: Map<VoiceId, Arc<VoiceControl>>,
    cache: ClipCache,
    tap: Arc<Tap>,
}

impl KiraBackend {
    pub fn open() -> Option<Self> {
        Self::with_backend(Default::default())
    }
}

impl<B: Backend> KiraBackend<B> {
    pub fn with_backend(settings: B::Settings) -> Option<Self> {
        let mut main = MainTrackBuilder::new().sound_capacity(VOICES_PER_GROUP);
        let master = MasterStrip::build(&mut main);
        let output = OutputStage::build(&mut main, SpeakerMode::default());
        let tap = Arc::new(Tap::default());
        main.add_effect(TapEffect(Arc::clone(&tap)));
        let mut manager = AudioManager::<B>::new(AudioManagerSettings {
            capacities: Capacities::default(),
            main_track_builder: main,
            internal_buffer_size: 128,
            backend_settings: settings,
        })
        .ok()?;
        Some(Self {
            groups: GroupTracks::new(&mut manager, master)?,
            manager,
            output,
            voices: Map::default(),
            cache: ClipCache::new(),
            tap,
        })
    }

    #[cfg(test)]
    pub fn device(&mut self) -> &mut B {
        self.manager.backend_mut()
    }

    #[cfg(test)]
    pub fn cache(&mut self) -> &mut ClipCache {
        &mut self.cache
    }
}

impl<B: Backend + Clocked> AudioBackend for KiraBackend<B> {
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

    fn set_reverb(&mut self, params: &ReverbParams) {
        self.groups.set_reverb(params);
    }

    fn tap(&self) -> Option<Arc<Tap>> {
        Some(Arc::clone(&self.tap))
    }

    fn advance(&mut self, seconds: f64) {
        self.groups.maintain();
        self.manager.backend_mut().advance(seconds);
    }
}

#[cfg(test)]
#[path = "backend_tests.rs"]
pub(crate) mod backend_tests;
