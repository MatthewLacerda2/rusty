//! The WAV encoder: `f32` samples → stereo 16-bit PCM RIFF bytes.
//!
//! Everything upstream works in `f32` in `−1..=1`; this is the *only* place the
//! signal is quantised. The output is deliberately the simplest thing an
//! ffmpeg decoder will take: **stereo, 16-bit PCM, uncompressed**. That is a
//! RIFF header and a sample loop — about sixty lines — so it earns no
//! dependency, and hand-rolling it keeps a bake byte-exact, where an encoder
//! library could shift the output from under the determinism promise on a
//! version bump.
//!
//! Bytes rather than a file, because this crate does no I/O: a caller writes
//! them wherever the project keeps its bakes.
//!
//! Layout written, in little-endian order: `RIFF` chunk → `WAVE` form → `fmt `
//! (PCM, 2 channels) → `data` (the samples, left first in each frame).

use crate::core::SAMPLE_RATE;
use crate::stereo::CHANNELS;

/// Bytes per sample in the written file (16-bit PCM).
const BYTES_PER_SAMPLE: u32 = 2;
/// Bytes per sample-frame: one sample per channel.
const BYTES_PER_FRAME: u32 = BYTES_PER_SAMPLE * CHANNELS as u32;
/// The `fmt ` chunk's body size for uncompressed PCM.
const FMT_CHUNK_LEN: u32 = 16;
/// Format tag 1 = uncompressed PCM.
const FORMAT_PCM: u16 = 1;
/// Bytes of header before the samples begin.
const HEADER_LEN: usize = 44;

/// Encodes `samples` — interleaved stereo, left first — as a complete 16-bit
/// PCM WAV file at [`SAMPLE_RATE`].
///
/// Samples are clamped to `−1..=1` and rounded to nearest, so a signal that
/// somehow survived the limiter cannot wrap around into a click.
pub(crate) fn encode(samples: &[f32]) -> Vec<u8> {
    let data_len = samples.len() as u32 * BYTES_PER_SAMPLE;
    let mut out = Vec::with_capacity(HEADER_LEN + data_len as usize);

    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVE");

    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&FMT_CHUNK_LEN.to_le_bytes());
    out.extend_from_slice(&FORMAT_PCM.to_le_bytes());
    out.extend_from_slice(&(CHANNELS as u16).to_le_bytes());
    out.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    out.extend_from_slice(&(SAMPLE_RATE * BYTES_PER_FRAME).to_le_bytes()); // byte rate
    out.extend_from_slice(&(BYTES_PER_FRAME as u16).to_le_bytes()); // block align
    out.extend_from_slice(&16u16.to_le_bytes()); // bits per sample

    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for &sample in samples {
        out.extend_from_slice(&to_i16(sample).to_le_bytes());
    }
    out
}

/// How many sample frames a file of `file_len` bytes carries.
///
/// Worked out from the length rather than read back from the header, because
/// this is only ever asked about a file [`encode`] produced: the length is the
/// one thing that cannot disagree with the samples.
pub(crate) fn frames_in(file_len: usize) -> usize {
    file_len.saturating_sub(HEADER_LEN) / BYTES_PER_FRAME as usize
}

/// How long a file of `file_len` bytes plays for, in seconds.
///
/// So a caller can record what it baked without decoding it again — the one
/// fact about a bake that nothing else on the asset carries, and the one an
/// agent needs to lay a clip of the right length.
pub fn seconds_in(file_len: usize) -> f64 {
    frames_in(file_len) as f64 / f64::from(SAMPLE_RATE)
}

/// Quantises one `f32` sample to 16-bit, clamping first so `±1.0` maps to the
/// endpoints instead of overflowing.
#[inline]
fn to_i16(sample: f32) -> i16 {
    (sample.clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_declares_stereo_16_bit_pcm_at_the_render_rate() {
        let bytes = encode(&[0.0; 4]);
        assert_eq!(&bytes[0..4], b"RIFF");
        assert_eq!(&bytes[8..12], b"WAVE");
        assert_eq!(&bytes[12..16], b"fmt ");
        assert_eq!(u16::from_le_bytes([bytes[20], bytes[21]]), FORMAT_PCM);
        assert_eq!(u16::from_le_bytes([bytes[22], bytes[23]]), 2, "stereo");
        let rate = u32::from_le_bytes([bytes[24], bytes[25], bytes[26], bytes[27]]);
        assert_eq!(rate, SAMPLE_RATE);
        let byte_rate = u32::from_le_bytes([bytes[28], bytes[29], bytes[30], bytes[31]]);
        assert_eq!(byte_rate, SAMPLE_RATE * 4, "two channels of two bytes");
        assert_eq!(u16::from_le_bytes([bytes[32], bytes[33]]), 4, "block align");
        assert_eq!(u16::from_le_bytes([bytes[34], bytes[35]]), 16, "bit depth");
    }

    #[test]
    fn chunk_sizes_match_the_payload() {
        let bytes = encode(&[0.0; 10]);
        let riff = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        let data = u32::from_le_bytes([bytes[40], bytes[41], bytes[42], bytes[43]]);
        assert_eq!(data, 20, "10 samples at 2 bytes each");
        assert_eq!(riff as usize, bytes.len() - 8);
        assert_eq!(bytes.len(), HEADER_LEN + 20);
    }

    /// A frame is a sample on each channel, so the left and right of one
    /// moment land side by side and the file says so.
    #[test]
    fn a_frame_is_written_left_then_right() {
        let bytes = encode(&[1.0, -1.0]);
        assert_eq!(bytes.len(), HEADER_LEN + 4, "one frame");
        assert_eq!(
            i16::from_le_bytes([bytes[44], bytes[45]]),
            i16::MAX,
            "left first"
        );
        assert_eq!(i16::from_le_bytes([bytes[46], bytes[47]]), -i16::MAX);
    }

    #[test]
    fn quantisation_clamps_instead_of_wrapping() {
        assert_eq!(to_i16(0.0), 0);
        assert_eq!(to_i16(1.0), i16::MAX);
        assert_eq!(to_i16(-1.0), -i16::MAX);
        assert_eq!(to_i16(9.0), i16::MAX, "a hot sample clamps, never wraps");
        assert_eq!(to_i16(-9.0), -i16::MAX);
    }

    #[test]
    fn the_length_of_a_file_says_how_long_it_plays() {
        let one_second = encode(&vec![0.0; SAMPLE_RATE as usize * 2]);
        assert_eq!(frames_in(one_second.len()), SAMPLE_RATE as usize);
        assert!((seconds_in(one_second.len()) - 1.0).abs() < 1e-9);
        assert_eq!(frames_in(encode(&[]).len()), 0);
        // Never a panic on something that is not one of ours, however short.
        assert_eq!(frames_in(3), 0);
    }

    #[test]
    fn an_empty_buffer_is_still_a_valid_file() {
        let bytes = encode(&[]);
        assert_eq!(bytes.len(), HEADER_LEN);
        assert_eq!(
            u32::from_le_bytes([bytes[40], bytes[41], bytes[42], bytes[43]]),
            0
        );
    }
}
