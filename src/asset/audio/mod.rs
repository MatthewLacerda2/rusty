//! src/asset/audio/ — audio import: one decoder, and MP3 → WAV on arrival (#385).
//!
//! **MP3 is a source format, never a shipped one.** Every `.mp3` that enters a
//! project — dropped in by hand, or handed over as bytes by a provider — is decoded
//! once, its encoder delay and padding trimmed, and written as a 16-bit PCM `.wav`
//! beside it; the `.mp3` is then removed. A one-shot so converted fires on its first
//! real sample and a loop has no silence at the seam, which an MP3 played directly
//! cannot promise without the gapless header being honoured on every play. OGG
//! (Vorbis) is a shipped format in its own right (music, long dialogue) and is never
//! converted.
//!
//! The decoder here is also the runtime's: `audio::device::decode` plays every clip
//! through [`decode_file`], so an MP3 that has not been converted yet (dropped in
//! while the editor runs, before the next refresh) still plays, trimmed the same way.
//!
//! Channels and rate are kept as the source has them, the way Unity's AudioImporter
//! does by default ("Force To Mono" off): the voice (#412) already plays mono and
//! stereo, folding a stereo clip toward its mono downmix as `spatialBlend` rises, so
//! forcing either would only lose information.
//!
//! Purity: symphonia and `std::fs` only — no kira, wgpu, egui or mlua.

mod import;
pub mod wav;

pub use import::{import_mp3_bytes, refresh, resolve_clip_path, Refresh};

use std::fs::File;
use std::io::Cursor;

use symphonia::core::codecs::CodecParameters;
use symphonia::core::formats::TrackType;
use symphonia::core::io::{MediaSource, MediaSourceStream};

/// Decoded PCM: interleaved `f32` samples in `-1..1`, plus their format.
#[derive(Clone, Debug, PartialEq)]
pub struct DecodedAudio {
    pub channels: u16,
    pub sample_rate: u32,
    pub samples: Vec<f32>,
}

impl DecodedAudio {
    /// Whole frames (one sample per channel) in the clip.
    pub fn frames(&self) -> usize {
        self.samples.len() / usize::from(self.channels.max(1))
    }
}

/// Decode the file at `path` (`.ogg` Vorbis / `.wav` / `.mp3`). Errs with a
/// human-readable reason on any open/decode failure.
pub fn decode_file(path: &str) -> Result<DecodedAudio, String> {
    let file = File::open(path).map_err(|e| e.to_string())?;
    decode_source(Box::new(file))
}

/// Decode an in-memory encoded file (a provider's response body, say).
pub fn decode_bytes(bytes: Vec<u8>) -> Result<DecodedAudio, String> {
    decode_source(Box::new(Cursor::new(bytes)))
}

/// Decode one stream to interleaved `f32`. Symphonia's probe sniffs the container
/// from the bytes, so an extension need not be trusted. Its decoders run gapless by
/// default: an MP3 carrying a LAME/Xing header comes out with the encoder delay and
/// padding already trimmed, sample-exact to what was encoded.
fn decode_source(source: Box<dyn MediaSource>) -> Result<DecodedAudio, String> {
    let stream = MediaSourceStream::new(source, Default::default());
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
    Ok(DecodedAudio {
        channels,
        sample_rate,
        samples,
    })
}

#[cfg(test)]
mod tests;
