//! Channels and rate stay as the source has them (Unity's AudioImporter, "Force To Mono" off).

mod import;
pub mod wav;

pub use import::{refresh, resolve_clip_path, Refresh};

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
    #[cfg(test)]
    pub fn frames(&self) -> usize {
        self.samples.len() / usize::from(self.channels.max(1))
    }
}

/// The runtime's decoder too (`audio::device::decode`).
pub fn decode_file(path: &str) -> Result<DecodedAudio, String> {
    let file = File::open(path).map_err(|e| e.to_string())?;
    decode_source(Box::new(file))
}

fn decode_bytes(bytes: Vec<u8>) -> Result<DecodedAudio, String> {
    decode_source(Box::new(Cursor::new(bytes)))
}

/// Symphonia decoders run gapless: an MP3 with a LAME/Xing header comes out with encoder delay
/// and padding trimmed.
fn decode_source(source: Box<dyn MediaSource>) -> Result<DecodedAudio, String> {
    let stream = MediaSourceStream::new(source, Default::default());
    // No extension hint: the probe sniffs the container from the bytes.
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
