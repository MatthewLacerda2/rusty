use rusty_asset::audio::resolve_clip_path;
use rusty_core::collections::Map;
use rusty_core::fs::written;
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

    pub fn channels(&self) -> u16 {
        self.channels
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// The decoded interleaved samples, in `-1..1`.
    pub fn samples(&self) -> &[f32] {
        &self.samples
    }

    pub fn frames(&self) -> usize {
        self.samples.len() / usize::from(self.channels)
    }

    /// Frame `index`'s samples (one per channel); empty past the end.
    pub fn frame(&self, index: usize) -> &[f32] {
        let n = usize::from(self.channels);
        self.samples.get(index * n..index * n + n).unwrap_or(&[])
    }
}

#[derive(Default)]
pub struct ClipCache {
    clips: Map<String, CachedClip>,
    seen: u64,
    failed: Map<String, Option<u64>>,
}

impl ClipCache {
    pub fn new() -> Self {
        Self {
            seen: written::written_since(u64::MAX).0,
            ..Self::default()
        }
    }

    #[cfg(test)]
    pub fn insert(&mut self, path: &str, clip: CachedClip) {
        self.clips.insert(path.to_string(), clip);
    }

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

fn stamp(path: &str) -> Option<u64> {
    std::fs::metadata(&*resolve_clip_path(path))
        .ok()
        .map(|m| m.len())
}

pub fn decode_file(path: &str) -> Result<CachedClip, String> {
    let audio = rusty_asset::audio::decode_file(&resolve_clip_path(path))?;
    Ok(CachedClip::new(
        audio.channels,
        audio.sample_rate,
        audio.samples,
    ))
}

#[cfg(test)]
#[path = "decode_tests.rs"]
mod decode_tests;
