//! src/audio/device/decode.rs — path-cached PCM decode (#212, #465).
//!
//! Decodes `.ogg` / `.wav` / `.mp3` files to in-memory PCM **once per path** and
//! caches the result, so replaying a clip (a footstep fired hundreds of times)
//! re-reads neither the disk nor the decoder. A cached clip is the interleaved `f32`
//! samples plus the channel count and sample rate: what a [`ClipSound`] plays from
//! and what `Sound.Level` (#378) meters.
//!
//! Decoding is symphonia's (the version kira renders with), through the asset
//! importer's decoder (`asset::audio`, #385) rather than kira's loader: kira keeps
//! only stereo frames, and the device's stereo rule needs to know a clip's real
//! channel count (a mono clip and a stereo one with equal channels pan differently
//! under `spatial_blend`), and it accepts clips of more than two channels. Platform layer only; the deterministic
//! sim never reaches it (the `NullBackend` decodes nothing).
//!
//! [`ClipSound`]: super::voice::ClipSound

use crate::core::collections::Map;
use std::sync::Arc;

/// Decoded PCM for one clip: interleaved `f32` samples + format. Cheap to clone
/// (the samples are shared behind an `Arc`), so every voice of the same clip plays
/// from one copy.
#[derive(Clone, Debug)]
pub struct CachedClip {
    channels: u16,
    sample_rate: u32,
    samples: Arc<[f32]>,
}

impl CachedClip {
    /// A clip over already-decoded interleaved samples (zimmer's in-memory output
    /// takes this path too). `channels` and `sample_rate` are clamped to at least 1.
    pub fn new(channels: u16, sample_rate: u32, samples: impl Into<Arc<[f32]>>) -> Self {
        Self {
            channels: channels.max(1),
            sample_rate: sample_rate.max(1),
            samples: samples.into(),
        }
    }

    /// Interleaved channel count.
    pub fn channels(&self) -> u16 {
        self.channels
    }

    /// Frames per second.
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// The decoded interleaved samples, in `-1..1`.
    pub fn samples(&self) -> &[f32] {
        &self.samples
    }

    /// Whole frames in the clip.
    pub fn frames(&self) -> usize {
        self.samples.len() / usize::from(self.channels)
    }

    /// Frame `index`'s samples (one per channel); empty past the end.
    pub fn frame(&self, index: usize) -> &[f32] {
        let n = usize::from(self.channels);
        self.samples.get(index * n..index * n + n).unwrap_or(&[])
    }
}

/// A path → decoded-PCM cache. One lives in the device backend; clips decode
/// lazily on first play and are reused thereafter.
#[derive(Default)]
pub struct ClipCache {
    clips: Map<String, Option<CachedClip>>,
}

impl ClipCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Cache an already-decoded clip under `path` (in-memory audio, such as a bake
    /// that never touched disk), so playing `path` plays it.
    pub fn insert(&mut self, path: &str, clip: CachedClip) {
        self.clips.insert(path.to_string(), Some(clip));
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

/// Decode one file to interleaved `f32` PCM through the asset importer's decoder
/// (#385), which trims an MP3's encoder delay and padding. A reference to an MP3
/// that has since been converted plays the `.wav` that replaced it; an MP3 not yet
/// converted still plays (the runtime fallback). Errs with a human-readable reason
/// on any open/decode failure.
pub fn decode_file(path: &str) -> Result<CachedClip, String> {
    let audio = crate::asset::audio::decode_file(&crate::asset::audio::resolve_clip_path(path))?;
    Ok(CachedClip::new(
        audio.channels,
        audio.sample_rate,
        audio.samples,
    ))
}

#[cfg(test)]
#[path = "decode_tests.rs"]
mod decode_tests;
