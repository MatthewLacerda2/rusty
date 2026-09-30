//! src/audio/device/rodio.rs — the real audio device (#212).
//!
//! The platform-layer [`AudioBackend`] backed by `rodio` (a `cpal` device + mixer).
//! Opened by the windowed shell (`shell/boot.rs`) and injected into the `AudioMaestro`
//! after `GameWorld::new`, so the device only ever exists in the windowed process —
//! never in the harness, which keeps the `NullBackend`.
//!
//! One `rodio::Sink` per live voice, playing its clip through a [`PanSource`]. Every
//! sink feeds one stereo mixer that reaches the device through the [`MasterBus`]
//! (#546), the mix's single output stage where the speaker mode shapes the total. The
//! maestro hands over a [`VoiceMix`] per voice each frame (#412): the gain (already
//! folded with master) and rate go to the sink, pan + `spatial_blend` to the voice's
//! shared [`PanControl`], and a paused mix pauses the sink so it resumes in place. A
//! looping voice repeats its decoded buffer; a one-shot plays once and the maestro
//! drops it. Decoding is path-cached (`ClipCache`), so the hundredth footstep costs
//! no disk or decode.

use std::collections::HashMap;
use std::sync::Arc;

use ::rodio::cpal::traits::HostTrait;
use ::rodio::dynamic_mixer::{self, DynamicMixerController};
use ::rodio::source::Source;
use ::rodio::{DeviceTrait, OutputStream, Sink};

use super::decode::ClipCache;
use super::master::{MasterBus, MasterControl};
use super::pan::{PanControl, PanSource};
use crate::audio::backend::{AudioBackend, PlayParams, VoiceId, VoiceMix};
use crate::audio::SpeakerMode;

/// The bus rate when the device will not say its own (the mixer resamples anyway).
const FALLBACK_RATE: u32 = 48_000;

/// One live voice: its sink (gain, speed, pause) plus the pan control its
/// [`PanSource`] reads on the audio thread.
struct Voice {
    sink: Sink,
    pan: Arc<PanControl>,
}

impl Voice {
    /// Apply `mix` in place — no restart, the voice keeps its playhead.
    fn apply(&self, mix: &VoiceMix) {
        self.sink.set_volume(mix.gain.max(0.0));
        self.pan.set(mix.pan, mix.spatial_blend);
        if mix.paused {
            self.sink.pause();
        } else {
            self.sink.set_speed(mix.speed.max(f32::EPSILON));
            self.sink.play();
        }
    }
}

/// `rodio`-backed mixer. Holds the output stream alive (`_stream`) for the life of
/// the backend — dropping it would silence everything.
pub struct RodioBackend {
    _stream: OutputStream,
    /// The master mixer's input: every voice's sink is added here.
    bus: Arc<DynamicMixerController<f32>>,
    master: Arc<MasterControl>,
    voices: HashMap<VoiceId, Voice>,
    cache: ClipCache,
}

impl RodioBackend {
    /// Open the default output device. Returns `None` when no device is available
    /// (a headless box, a CI runner) so the caller can fall back to the
    /// `NullBackend` instead of failing the whole app.
    pub fn open() -> Option<Self> {
        let (stream, handle) = OutputStream::try_default().ok()?;
        let (bus, mixer) = dynamic_mixer::mixer(2, device_rate());
        let master = MasterControl::new(SpeakerMode::default());
        handle
            .play_raw(MasterBus::new(mixer, Arc::clone(&master)))
            .ok()?;
        Some(Self {
            _stream: stream,
            bus,
            master,
            voices: HashMap::new(),
            cache: ClipCache::new(),
        })
    }
}

impl AudioBackend for RodioBackend {
    fn play(&mut self, id: VoiceId, params: &PlayParams) -> bool {
        let Some(clip) = self.cache.get_or_decode(&params.clip) else {
            return false;
        };
        let (sink, output) = Sink::new_idle();
        self.bus.add(output);
        let voice = Voice {
            sink,
            pan: PanControl::new(params.mix.pan, params.mix.spatial_blend),
        };
        // Mix before appending so the first samples already play at the resolved
        // state (a paused voice starts paused).
        voice.apply(&params.mix);
        let pan = Arc::clone(&voice.pan);
        if params.looping {
            voice.sink.append(PanSource::new(
                clip.source().convert_samples().repeat_infinite(),
                pan,
            ));
        } else {
            voice
                .sink
                .append(PanSource::new(clip.source().convert_samples(), pan));
        }
        // A previous voice on the same id (re-`Play`) is replaced; dropping its sink
        // stops it.
        self.voices.insert(id, voice);
        true
    }

    fn stop(&mut self, id: VoiceId) {
        if let Some(voice) = self.voices.remove(&id) {
            voice.sink.stop();
        }
    }

    fn set_mix(&mut self, id: VoiceId, mix: &VoiceMix) {
        if let Some(voice) = self.voices.get(&id) {
            voice.apply(mix);
        }
    }

    fn is_live(&self, id: VoiceId) -> bool {
        self.voices.get(&id).is_some_and(|v| !v.sink.empty())
    }

    fn stop_all(&mut self) {
        for (_, voice) in self.voices.drain() {
            voice.sink.stop();
        }
    }

    fn set_speaker_mode(&mut self, mode: SpeakerMode) {
        self.master.set(mode);
    }
}

/// The default output device's sample rate — the one `OutputStream::try_default`
/// opens at — so the bus runs at the device rate and is not resampled twice.
fn device_rate() -> u32 {
    ::rodio::cpal::default_host()
        .default_output_device()
        .and_then(|d| d.default_output_config().ok())
        .map_or(FALLBACK_RATE, |c| c.sample_rate().0)
}
