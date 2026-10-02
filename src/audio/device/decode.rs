//! src/audio/device/decode.rs — path-cached PCM decode (#212, #465).
//!
//! Decodes `.ogg` / `.wav` / `.mp3` files to in-memory PCM **once per path** and
//! caches the result, so replaying a clip (a footstep fired hundreds of times)
//! re-reads neither the disk nor the decoder. A cached clip is the interleaved `f32`
//! samples plus the channel count and sample rate: what a [`ClipSound`] plays from
//! and what `Sound.Level` (#378) meters.
//!
//! Decoding is symphonia's (the version kira renders with), read directly rather
//! than through kira's loader: kira keeps only stereo frames, and the device's
//! stereo rule needs to know a clip's real channel count (a mono clip and a
//! stereo one with equal channels pan differently under `spatial_blend`), and it
//! accepts clips of more than two channels. Platform layer only; the deterministic
//! sim never reaches it (the `NullBackend` decodes nothing).
//!
//! [`ClipSound`]: super::voice::ClipSound

use std::collections::HashMap;
use std::fs::File;
use std::sync::Arc;

use symphonia::core::codecs::CodecParameters;
use symphonia::core::formats::TrackType;
use symphonia::core::io::MediaSourceStream;

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
    clips: HashMap<String, Option<CachedClip>>,
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

/// Decode one file to interleaved `f32` PCM. Symphonia's probe sniffs the container
/// (`.ogg` Vorbis / `.wav` / `.mp3`) from the stream, so the extension need not be
/// trusted. Errs with a human-readable reason on any open/decode failure.
pub fn decode_file(path: &str) -> Result<CachedClip, String> {
    let file = File::open(path).map_err(|e| e.to_string())?;
    let stream = MediaSourceStream::new(Box::new(file), Default::default());
    let mut reader = symphonia::default::get_probe()
        .probe(
            &Default::default(),
            stream,
            Default::default(),
            Default::default(),
        )
        .map_err(|e| e.to_string())?;
    let track = reader
        .default_track(TrackType::Audio)
        .ok_or("no audio track")?;
    let track_id = track.id;
    let Some(CodecParameters::Audio(params)) = track.codec_params.as_ref() else {
        return Err("no audio track".to_string());
    };
    let mut decoder = symphonia::default::get_codecs()
        .make_audio_decoder(params, &Default::default())
        .map_err(|e| e.to_string())?;
    let (mut samples, mut chunk) = (Vec::new(), Vec::new());
    let (mut channels, mut sample_rate) = (0, params.sample_rate.unwrap_or(0));
    while let Some(packet) = reader.next_packet().map_err(|e| e.to_string())? {
        if packet.track_id != track_id {
            continue;
        }
        let buffer = decoder.decode(&packet).map_err(|e| e.to_string())?;
        channels = buffer.spec().channels().count();
        sample_rate = buffer.spec().rate();
        chunk.clear();
        buffer.copy_to_vec_interleaved(&mut chunk);
        samples.extend_from_slice(&chunk);
    }
    if samples.is_empty() || channels == 0 {
        return Err("no samples".to_string());
    }
    let channels = u16::try_from(channels).map_err(|_| "too many channels")?;
    Ok(CachedClip::new(channels, sample_rate, samples))
}

#[cfg(test)]
#[path = "decode_tests.rs"]
mod decode_tests;
