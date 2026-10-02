//! Decode tests (#212, #545, #465): every container the engine plays, and the cache.

use super::*;

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/audio/device/fixtures");

/// The peak absolute sample of `channel` in `clip`.
fn peak(clip: &CachedClip, channel: usize) -> f32 {
    (0..clip.frames())
        .map(|i| clip.frame(i)[channel].abs())
        .fold(0.0, f32::max)
}

#[test]
fn missing_file_decodes_to_none_and_is_cached() {
    let mut cache = ClipCache::new();
    assert!(cache.get_or_decode("does/not/exist.ogg").is_none());
    // Second call hits the cached `None` (no re-attempt); still `None`.
    assert!(cache.get_or_decode("does/not/exist.ogg").is_none());
    assert!(cache.clips.contains_key("does/not/exist.ogg"));
}

#[test]
fn a_cached_clip_is_decoded_once_and_shared() {
    let mut cache = ClipCache::new();
    let path = format!("{FIXTURES}/tone.mp3");
    let a = cache.get_or_decode(&path).unwrap();
    let b = cache.get_or_decode(&path).unwrap();
    assert!(Arc::ptr_eq(&a.samples, &b.samples), "one decode, shared");
}

/// 250 ms of a 440 Hz sine, mono (LAME resamples it to 22.05 kHz at 32 kbps).
#[test]
fn mp3_decodes_to_the_tone_it_was_encoded_from() {
    let clip = decode_file(&format!("{FIXTURES}/tone.mp3")).expect("MP3 decodes (#545)");
    assert_eq!((clip.channels(), clip.sample_rate()), (1, 22_050));
    // Roughly the encoded 250 ms (the encoder pads a frame or two either side).
    let secs = clip.frames() as f32 / 22_050.0;
    assert!((0.24..0.35).contains(&secs), "{secs} s decoded");
    // A real tone, not decoded silence: peaks near the half-scale amplitude.
    let peak = peak(&clip, 0);
    assert!((0.37..0.61).contains(&peak), "peak {peak}");
}

#[test]
fn ogg_vorbis_decodes_both_channels() {
    let clip = decode_file(&format!("{FIXTURES}/tone.ogg")).expect("OGG decodes");
    assert_eq!((clip.channels(), clip.sample_rate()), (2, 22_050));
    let (left, right) = (peak(&clip, 0), peak(&clip, 1));
    assert!((0.4..0.6).contains(&left), "left {left}");
    assert!((0.2..0.3).contains(&right), "right {right}");
}

/// A minimal 16-bit PCM WAV: the header plus `samples`.
fn wav_bytes(channels: u16, rate: u32, samples: &[i16]) -> Vec<u8> {
    let data = (samples.len() * 2) as u32;
    let mut out = Vec::new();
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&(rate * u32::from(channels) * 2).to_le_bytes());
    out.extend_from_slice(&(channels * 2).to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data.to_le_bytes());
    for s in samples {
        out.extend_from_slice(&s.to_le_bytes());
    }
    out
}

#[test]
fn wav_decodes_sample_exact_and_keeps_more_than_two_channels() {
    let path = crate::test_temp::dir().join("rusty_decode_quad.wav");
    let samples = [16_384, -16_384, 8_192, 0, 0, 0, 0, 32_767];
    std::fs::write(&path, wav_bytes(4, 8_000, &samples)).unwrap();
    let clip = decode_file(path.to_str().unwrap()).expect("WAV decodes");
    std::fs::remove_file(&path).ok();
    assert_eq!(
        (clip.channels(), clip.sample_rate(), clip.frames()),
        (4, 8_000, 2)
    );
    assert_eq!(&clip.frame(0)[..3], &[0.5, -0.5, 0.25]);
    assert!(clip.frame(2).is_empty(), "past the end");
}
