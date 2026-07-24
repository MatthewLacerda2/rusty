//! src/soundgen/wav.rs — the WAV writer: `f32` buffer → mono 16-bit PCM RIFF
//! (#357).
//!
//! Everything upstream works in `f32` in `−1..=1`; this is the *only* place the
//! signal is quantized. The output is deliberately the simplest thing the engine's
//! playback already decodes: **mono, 16-bit PCM, 44.1 kHz**, uncompressed. That is a
//! RIFF header and a sample loop — about sixty lines — so it earns no dependency,
//! and hand-rolling it keeps the bake byte-exact (no encoder version can shift the
//! output from under the determinism promise).
//!
//! Layout written, in little-endian order: `RIFF` chunk → `WAVE` form → `fmt `
//! (PCM, 1 channel) → `data` (the samples).

use std::io::Write;

/// Bytes per sample in the written file (16-bit PCM).
const BYTES_PER_SAMPLE: u32 = 2;
/// The `fmt ` chunk's body size for uncompressed PCM.
const FMT_CHUNK_LEN: u32 = 16;
/// Format tag 1 = uncompressed PCM.
const FORMAT_PCM: u16 = 1;

/// Encode `samples` as a complete mono 16-bit PCM WAV file at `sample_rate`.
///
/// Samples are clamped to `−1..=1` and rounded to nearest, so a signal that
/// survived the limiter cannot wrap around into a click.
pub fn encode(samples: &[f32], sample_rate: u32) -> Vec<u8> {
    let data_len = samples.len() as u32 * BYTES_PER_SAMPLE;
    let mut out = Vec::with_capacity(44 + data_len as usize);

    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVE");

    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&FMT_CHUNK_LEN.to_le_bytes());
    out.extend_from_slice(&FORMAT_PCM.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // channels: mono
    out.extend_from_slice(&sample_rate.to_le_bytes());
    out.extend_from_slice(&(sample_rate * BYTES_PER_SAMPLE).to_le_bytes()); // byte rate
    out.extend_from_slice(&(BYTES_PER_SAMPLE as u16).to_le_bytes()); // block align
    out.extend_from_slice(&16u16.to_le_bytes()); // bits per sample

    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for &s in samples {
        out.extend_from_slice(&to_i16(s).to_le_bytes());
    }
    out
}

/// Quantize one `f32` sample to 16-bit, clamping first so `±1.0` maps to the
/// endpoints instead of overflowing.
#[inline]
fn to_i16(s: f32) -> i16 {
    (s.clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16
}

/// The directory `path` implies, or `None` when it names a file in the current
/// directory. `Path::parent` reports a bare filename's parent as the *empty* path,
/// which is not a directory anyone can create — so the two cases are folded here
/// rather than left as a nested condition at the call site.
fn parent_dir(path: &str) -> Option<&std::path::Path> {
    std::path::Path::new(path)
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
}

/// Ensure the directory `path` lives in exists, creating it if needed. A bare
/// filename has no directory to make, so it succeeds having touched nothing —
/// which is why this is a function and not an `if` inside [`write`]: the
/// do-nothing case is a real outcome worth asserting without writing a file.
fn create_parent_dir(path: &str) -> Result<(), String> {
    match parent_dir(path) {
        Some(parent) => std::fs::create_dir_all(parent).map_err(|e| e.to_string()),
        None => Ok(()),
    }
}

/// Encode `samples` and write them to `path`, returning the path on success. Any
/// missing directories along `path` are created, so an agent can bake straight into
/// a workspace folder that doesn't exist yet.
pub fn write(samples: &[f32], sample_rate: u32, path: &str) -> Result<String, String> {
    create_parent_dir(path)?;
    let bytes = encode(samples, sample_rate);
    let mut file = std::fs::File::create(path).map_err(|e| e.to_string())?;
    file.write_all(&bytes).map_err(|e| e.to_string())?;
    Ok(path.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_declares_mono_16_bit_pcm_at_the_given_rate() {
        let bytes = encode(&[0.0; 4], 44_100);
        assert_eq!(&bytes[0..4], b"RIFF");
        assert_eq!(&bytes[8..12], b"WAVE");
        assert_eq!(&bytes[12..16], b"fmt ");
        assert_eq!(u16::from_le_bytes([bytes[20], bytes[21]]), FORMAT_PCM);
        assert_eq!(u16::from_le_bytes([bytes[22], bytes[23]]), 1, "mono");
        let rate = u32::from_le_bytes([bytes[24], bytes[25], bytes[26], bytes[27]]);
        assert_eq!(rate, 44_100);
        assert_eq!(u16::from_le_bytes([bytes[34], bytes[35]]), 16, "bit depth");
    }

    #[test]
    fn chunk_sizes_match_the_payload() {
        let bytes = encode(&[0.0; 10], 44_100);
        let riff = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        let data = u32::from_le_bytes([bytes[40], bytes[41], bytes[42], bytes[43]]);
        assert_eq!(data, 20, "10 mono samples at 2 bytes each");
        assert_eq!(riff as usize, bytes.len() - 8);
        assert_eq!(bytes.len(), 44 + 20);
    }

    #[test]
    fn quantization_clamps_instead_of_wrapping() {
        assert_eq!(to_i16(0.0), 0);
        assert_eq!(to_i16(1.0), i16::MAX);
        assert_eq!(to_i16(-1.0), -i16::MAX);
        assert_eq!(to_i16(9.0), i16::MAX, "a hot sample clamps, never wraps");
        assert_eq!(to_i16(-9.0), -i16::MAX);
    }

    #[test]
    fn a_bare_filename_implies_no_directory_to_create() {
        assert_eq!(parent_dir("beep.wav"), None, "nothing to create in the cwd");
        assert_eq!(
            parent_dir("sounds/sfx/beep.wav"),
            Some(std::path::Path::new("sounds/sfx")),
        );
        // And that is a success, not an error — baking to a bare filename is a
        // perfectly ordinary call. Asserted here rather than by calling `write`,
        // which would have to litter the process's working directory to prove it.
        assert_eq!(create_parent_dir("beep.wav"), Ok(()));
    }

    #[test]
    fn write_creates_missing_parent_directories() {
        let dir = std::env::temp_dir().join("rusty_soundgen_wav_dir");
        std::fs::remove_dir_all(&dir).ok();
        let path = dir.join("beep.wav");
        let p = path.to_str().expect("utf-8 temp path");
        assert_eq!(write(&[0.5, -0.5], 44_100, p).unwrap(), p);
        assert_eq!(std::fs::read(p).unwrap().len(), 48);
        std::fs::remove_dir_all(&dir).ok();
    }
}
