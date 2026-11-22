//! Same 16-bit PCM as zimmer's bake; its encoder is crate-private and stereo-only, hence this one.

const BYTES_PER_SAMPLE: u16 = 2;
const FORMAT_PCM: u16 = 1;

/// Samples are clamped and rounded, so an overshoot cannot wrap into a click.
pub fn encode(channels: u16, sample_rate: u32, samples: &[f32]) -> Vec<u8> {
    let channels = channels.max(1);
    let block_align = channels * BYTES_PER_SAMPLE;
    let data_len = (samples.len() * usize::from(BYTES_PER_SAMPLE)) as u32;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&FORMAT_PCM.to_le_bytes());
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&sample_rate.to_le_bytes());
    out.extend_from_slice(&(sample_rate * u32::from(block_align)).to_le_bytes());
    out.extend_from_slice(&block_align.to_le_bytes());
    out.extend_from_slice(&(BYTES_PER_SAMPLE * 8).to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for &sample in samples {
        out.extend_from_slice(&to_i16(sample).to_le_bytes());
    }
    out
}

fn to_i16(sample: f32) -> i16 {
    (sample.clamp(-1.0, 1.0) * f32::from(i16::MAX)).round() as i16
}
