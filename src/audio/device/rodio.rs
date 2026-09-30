//! src/audio/device/rodio.rs — the real audio device (#212).
//!
//! The platform-layer [`AudioBackend`] backed by `rodio` (a `cpal` device + mixer).
//! Opened by the windowed shell (`shell/boot.rs`) and injected into the `AudioMaestro`
//! after `GameWorld::new`, so the device only ever exists in the windowed process —
//! never in the harness, which keeps the `NullBackend`.
//!
//! One `rodio::Sink` per live voice, playing its clip through a [`PanSource`]. The
//! maestro hands over a [`VoiceMix`] per voice each frame (#412): the gain (already
//! folded with master) and rate go to the sink, pan + `spatial_blend` to the voice's
//! shared [`PanControl`], and a paused mix pauses the sink so it resumes in place. A
//! looping voice repeats its decoded buffer; a one-shot plays once and the maestro
//! drops it. Decoding is path-cached (`ClipCache`), so the hundredth footstep costs
//! no disk or decode.

use std::collections::HashMap;
use std::sync::Arc;

use ::rodio::source::Source;
use ::rodio::{OutputStream, OutputStreamHandle, Sink};

use super::decode::ClipCache;
use super::pan::{PanControl, PanSource};
use crate::audio::backend::{AudioBackend, PlayParams, VoiceId, VoiceMix};

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
    handle: OutputStreamHandle,
    voices: HashMap<VoiceId, Voice>,
    cache: ClipCache,
}

impl RodioBackend {
    /// Open the default output device. Returns `None` when no device is available
    /// (a headless box, a CI runner) so the caller can fall back to the
    /// `NullBackend` instead of failing the whole app.
    pub fn open() -> Option<Self> {
        let (stream, handle) = OutputStream::try_default().ok()?;
        Some(Self {
            _stream: stream,
            handle,
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
        let Ok(sink) = Sink::try_new(&self.handle) else {
            return false;
        };
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
}
