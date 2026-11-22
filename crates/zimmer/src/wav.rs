//! Hand-rolled, not a crate, so no encoder's version bump can move a bake's bytes.

use crate::core::SAMPLE_RATE;
use crate::stereo::CHANNELS;

// Byte counts and the format tag, as the RIFF/WAVE header for 16-bit PCM fixes them.
const BYTES_PER_SAMPLE: u32 = 2;
const BYTES_PER_FRAME: u32 = BYTES_PER_SAMPLE * CHANNELS as u32;
const FMT_CHUNK_LEN: u32 = 16;
const FORMAT_PCM: u16 = 1;
const HEADER_LEN: usize = 44;

pub(crate) fn encode(samples: &[f32]) -> Vec<u8> {
    let data_len = samples.len() as u32 * BYTES_PER_SAMPLE;
    let mut out = Vec::with_capacity(HEADER_LEN + data_len as usize);

    out.extend_from_slice(b"RIFF");
    // The bytes after this field: the 44-byte header less `RIFF` and the field itself.
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVE");

    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&FMT_CHUNK_LEN.to_le_bytes());
    out.extend_from_slice(&FORMAT_PCM.to_le_bytes());
    out.extend_from_slice(&(CHANNELS as u16).to_le_bytes());
    out.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    out.extend_from_slice(&(SAMPLE_RATE * BYTES_PER_FRAME).to_le_bytes());
    out.extend_from_slice(&(BYTES_PER_FRAME as u16).to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes()); // bits per sample

    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for &sample in samples {
        out.extend_from_slice(&to_i16(sample).to_le_bytes());
    }
    out
}

/// From the length, not the header: only asked of a file [`encode`] wrote, whose length cannot
/// disagree with its samples.
pub(crate) fn frames_in(file_len: usize) -> usize {
    file_len.saturating_sub(HEADER_LEN) / BYTES_PER_FRAME as usize
}

/// A test hook: only `crates/zimmer/tests/suite` and its own unit test call it.
pub fn seconds_in(file_len: usize) -> f64 {
    frames_in(file_len) as f64 / f64::from(SAMPLE_RATE)
}

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
