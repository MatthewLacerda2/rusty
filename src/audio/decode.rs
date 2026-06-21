//! src/audio/decode.rs — path-cached PCM decode (#212).
//!
//! Decodes `.ogg` / `.wav` files to in-memory PCM **once per path** and caches the
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
    /// cached clip, or `None` when the file can't be opened/decoded.
    pub fn get_or_decode(&mut self, path: &str) -> Option<CachedClip> {
        if let Some(entry) = self.clips.get(path) {
            return entry.clone();
        }
        let decoded = decode_file(path);
        self.clips.insert(path.to_string(), decoded.clone());
        decoded
    }
}

/// Decode one file to interleaved `i16` PCM. `rodio`'s `Decoder` sniffs the
/// container (`.ogg` Vorbis / `.wav`) from the stream, so the extension need not be
/// trusted. Returns `None` on any open/decode error.
fn decode_file(path: &str) -> Option<CachedClip> {
    let file = File::open(path).ok()?;
    let decoder = Decoder::new(BufReader::new(file)).ok()?;
    let channels = decoder.channels();
    let sample_rate = decoder.sample_rate();
    let samples: Vec<i16> = decoder.collect();
    if samples.is_empty() {
        return None;
    }
    Some(CachedClip {
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
