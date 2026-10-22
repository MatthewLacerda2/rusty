//! src/audio/device/decode.rs — path-cached PCM decode (#212, #465).
//!
//! Decodes `.ogg` / `.wav` / `.mp3` files to in-memory PCM **once per path** and
//! caches the result, so replaying a clip (a footstep fired hundreds of times)
//! re-reads neither the disk nor the decoder until the engine re-writes the file
//! (a `Sound.*` re-bake, #1208). A clip that failed to decode is retried only once
//! its file changes. A cached clip is the interleaved `f32`
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

use crate::asset::audio::resolve_clip_path;
use crate::core::collections::Map;
use crate::core::fs::written;
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
/// lazily on first play and are reused thereafter, until the engine re-writes the
/// file (a `Sound.*` re-bake to the same path, #1208): each lookup first drains
/// [`written`]'s notes and drops what they name. A failed decode is not cached: the
/// next play retries it once the file has changed, so a clip played before it was
/// baked plays once it is, and a corrupt clip fired every frame costs one `stat`
/// per play, not a read and a decode.
#[derive(Default)]
pub struct ClipCache {
    clips: Map<String, CachedClip>,
    /// The newest write generation already applied to `clips`.
    seen: u64,
    /// Paths whose decode failed (the warning is logged once per entry), with the
    /// file's [`stamp`] at that time: retried only when the stamp moves or the
    /// write log names the path.
    failed: Map<String, Option<u64>>,
}

impl ClipCache {
    pub fn new() -> Self {
        Self {
            seen: written::written_since(u64::MAX).0,
            ..Self::default()
        }
    }

    /// Cache an already-decoded clip under `path`, so playing `path` plays it: a
    /// test's in-memory audio.
    #[cfg(test)]
    pub fn insert(&mut self, path: &str, clip: CachedClip) {
        self.clips.insert(path.to_string(), clip);
    }

    /// The clip at `path`, decoded on first request and cached. Returns `None` when
    /// the file can't be opened/decoded — logged once per path, so an unplayable
    /// clip is never silent *and* unexplained — and retried once the file changes.
    pub fn get_or_decode(&mut self, path: &str) -> Option<CachedClip> {
        self.drop_rewritten();
        if let Some(clip) = self.clips.get(path) {
            return Some(clip.clone());
        }
        let now = stamp(path);
        if self.failed.get(path) == Some(&now) {
            return None;
        }
        match decode_file(path) {
            Ok(clip) => {
                self.failed.remove(path);
                self.clips.insert(path.to_string(), clip.clone());
                Some(clip)
            }
            Err(why) => {
                if self.failed.insert(path.to_string(), now).is_none() {
                    log::warn!("[Audio] clip '{path}' did not decode ({why}); it plays silence");
                }
                None
            }
        }
    }

    /// Forget every clip (and failure) whose file the engine wrote since the last
    /// lookup. A key is matched as written and as it resolves, so a `foo.mp3` that
    /// plays the `foo.wav` an import replaced it with follows that WAV's notes.
    fn drop_rewritten(&mut self) {
        let (seen, paths) = written::written_since(self.seen);
        self.seen = seen;
        let stale = written::Stale::new(paths);
        if stale.is_empty() {
            return;
        }
        let is_stale = |key: &str| stale.matches(key) || stale.matches(&resolve_clip_path(key));
        self.clips.retain(|key, _| !is_stale(key));
        self.failed.retain(|key, _| !is_stale(key));
    }
}

/// What a failed clip's retry watches: the byte length of the file it resolves to,
/// `None` while missing. A length, not a modification time: `audio` is a sim row,
/// where `SystemTime` is banned (#757), and an engine write is caught by the write
/// log anyway, so only an outside tool rewriting a broken file at the same length
/// is missed.
fn stamp(path: &str) -> Option<u64> {
    std::fs::metadata(&*resolve_clip_path(path))
        .ok()
        .map(|m| m.len())
}

/// Decode one file to interleaved `f32` PCM through the asset importer's decoder
/// (#385), which trims an MP3's encoder delay and padding. A reference to an MP3
/// that has since been converted plays the `.wav` that replaced it; an MP3 not yet
/// converted still plays (the runtime fallback). Errs with a human-readable reason
/// on any open/decode failure.
pub fn decode_file(path: &str) -> Result<CachedClip, String> {
    let audio = crate::asset::audio::decode_file(&resolve_clip_path(path))?;
    Ok(CachedClip::new(
        audio.channels,
        audio.sample_rate,
        audio.samples,
    ))
}

#[cfg(test)]
#[path = "decode_tests.rs"]
mod decode_tests;
