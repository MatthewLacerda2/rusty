//! src/audio/rodio_backend.rs — the real audio device (#212).
//!
//! The platform-layer [`AudioBackend`] backed by `rodio` (a `cpal` device + mixer).
//! Opened by the windowed app in `main.rs` and injected into the `AudioMaestro`
//! after `GameWorld::new`, so the device only ever exists in the windowed process —
//! never in the harness, which keeps the `NullBackend`.
//!
//! One `rodio::Sink` per live voice. A voice's volume is the per-source gain folded
//! with the master volume (the maestro does the fold and hands the result here). A
//! looping voice repeats its decoded buffer; a one-shot plays once and the maestro
//! drops it. Decoding is path-cached (`ClipCache`), so the hundredth footstep costs
//! no disk or decode.

use std::collections::HashMap;

use rodio::source::Source;
use rodio::{OutputStream, OutputStreamHandle, Sink};

use super::backend::{AudioBackend, PlayParams, VoiceId};
use super::decode::ClipCache;

/// `rodio`-backed mixer. Holds the output stream alive (`_stream`) for the life of
/// the backend — dropping it would silence everything.
pub struct RodioBackend {
    _stream: OutputStream,
    handle: OutputStreamHandle,
    sinks: HashMap<VoiceId, Sink>,
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
            sinks: HashMap::new(),
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
        sink.set_volume(params.volume.max(0.0));
        if params.looping {
            sink.append(clip.source().repeat_infinite());
        } else {
            sink.append(clip.source());
        }
        // A previous voice on the same id (re-`Play`) is replaced; dropping its sink
        // stops it.
        self.sinks.insert(id, sink);
        true
    }

    fn stop(&mut self, id: VoiceId) {
        if let Some(sink) = self.sinks.remove(&id) {
            sink.stop();
        }
    }

    fn set_volume(&mut self, id: VoiceId, volume: f32) {
        if let Some(sink) = self.sinks.get(&id) {
            sink.set_volume(volume.max(0.0));
        }
    }

    fn set_master_volume(&mut self, _master: f32, voices: &[(VoiceId, f32)]) {
        // The maestro has already folded master into each per-voice volume; apply.
        for (id, folded) in voices {
            if let Some(sink) = self.sinks.get(id) {
                sink.set_volume(folded.max(0.0));
            }
        }
    }

    fn stop_all(&mut self) {
        for (_, sink) in self.sinks.drain() {
            sink.stop();
        }
    }
}
