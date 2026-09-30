//! src/audio/device/decode.rs — path-cached PCM decode (#212).
//!
//! Decodes `.ogg` / `.wav` / `.mp3` files to in-memory PCM **once per path** and caches the
//! result, so replaying a clip (a footstep fired hundreds of times) re-reads neither
//! the disk nor the decoder. A cached clip is the raw interleaved `i16` samples plus
//! the channel count and sample rate — everything `rodio` needs to build a fresh
//! playable source on each play.
//!
//! This module depends on `rodio`/`symphonia`, so it lives with the real backend in
//! the platform layer; the deterministic sim never reaches it (the `NullBackend`
//! decodes nothing).

use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::sync::Arc;

use rodio::buffer::SamplesBuffer;
use rodio::{Decoder, Source};

/// Decoded PCM for one clip: interleaved `i16` samples + format. Cheap to clone
/// (the sample vector is shared behind an `Arc`), so each play builds its own
/// independent `SamplesBuffer` over the same data.
#[derive(Clone)]
pub struct CachedClip {
    channels: u16,
    sample_rate: u32,
    samples: Arc<Vec<i16>>,
}

impl CachedClip {
    /// Build a fresh playable `rodio` source over the cached samples. Each call
    /// yields an independent source, so the same clip can play overlapping voices.
    pub fn source(&self) -> SamplesBuffer<i16> {
        SamplesBuffer::new(self.channels, self.sample_rate, (*self.samples).clone())
    }
}

/// A path → decoded-PCM cache. One lives in the `RodioBackend`; clips decode lazily
/// on first play and are reused thereafter.
#[derive(Default)]
pub struct ClipCache {
    clips: HashMap<String, Option<CachedClip>>,
}

impl ClipCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Decode `path` to PCM, caching the result (including a cached *failure* as
    /// `None`, so a missing/garbage file isn't retried every play). Returns the
    /// cached clip, or `None` when the file can't be opened/decoded — logged once,
    /// on the first attempt, so an unplayable clip is never silent *and* unexplained.
    pub fn get_or_decode(&mut self, path: &str) -> Option<CachedClip> {
        if let Some(entry) = self.clips.get(path) {
            return entry.clone();
        }
        let decoded = decode_file(path);
        if let Err(why) = &decoded {
            log::warn!("[Audio] clip '{path}' did not decode ({why}); it will play silence");
        }
        let decoded = decoded.ok();
        self.clips.insert(path.to_string(), decoded.clone());
        decoded
    }
}

/// Decode one file to interleaved `i16` PCM. `rodio`'s `Decoder` sniffs the
/// container (`.ogg` Vorbis / `.wav` / `.mp3`) from the stream, so the extension need not be
/// trusted. Errs with a human-readable reason on any open/decode failure.
fn decode_file(path: &str) -> Result<CachedClip, String> {
    let file = File::open(path).map_err(|e| e.to_string())?;
    let decoder = Decoder::new(BufReader::new(file)).map_err(|e| e.to_string())?;
    let channels = decoder.channels();
    let sample_rate = decoder.sample_rate();
    let samples: Vec<i16> = decoder.collect();
    if samples.is_empty() {
        return Err("no samples".to_string());
    }
    Ok(CachedClip {
        channels,
        sample_rate,
        samples: Arc::new(samples),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_decodes_to_none_and_is_cached() {
        let mut cache = ClipCache::new();
        assert!(cache.get_or_decode("does/not/exist.ogg").is_none());
        // Second call hits the cached `None` (no re-attempt); still `None`.
        assert!(cache.get_or_decode("does/not/exist.ogg").is_none());
        assert!(cache.clips.contains_key("does/not/exist.ogg"));
    }

    /// 250 ms of a 440 Hz sine, mono (LAME resamples it to 22.05 kHz at 32 kbps);
    /// regenerate with
    /// `fixtures/make_tone_mp3.py`.
    const TONE_MP3: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/audio/device/fixtures/tone.mp3"
    );

    #[test]
    fn mp3_decodes_to_the_tone_it_was_encoded_from() {
        let clip = ClipCache::new()
            .get_or_decode(TONE_MP3)
            .expect("the MP3 decoder is enabled (#545)");
        assert_eq!((clip.channels, clip.sample_rate), (1, 22_050));
        // Roughly the encoded 250 ms (the encoder pads a frame or two either side).
        let secs = clip.samples.len() as f32 / 22_050.0;
        assert!((0.24..0.35).contains(&secs), "{secs} s decoded");
        // A real tone, not decoded silence: peaks near the half-scale amplitude.
        let peak = clip.samples.iter().map(|s| s.unsigned_abs()).max().unwrap();
        assert!((12_000..20_000).contains(&peak), "peak {peak}");
    }

    #[test]
    fn cached_clip_builds_independent_sources() {
        let clip = CachedClip {
            channels: 1,
            sample_rate: 44_100,
            samples: Arc::new(vec![0, 1, 2, 3]),
        };
        let a = clip.source();
        let b = clip.source();
        assert_eq!(a.channels(), 1);
        assert_eq!(b.sample_rate(), 44_100);
    }
}
