//! Audio import tests (#385): the gapless trim, the WAV writer, and the refresh.

use std::f32::consts::TAU;
use std::path::Path;

use super::*;

const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/src/asset/audio/fixtures/loop.mp3"
);
/// The fixture's source: 0.5 s of 0.5 * sin(2π·440·t) at 44.1 kHz, mono. 440 Hz fits
/// exactly 220 cycles in it, so the tone loops without a seam.
const RATE: u32 = 44_100;
const FRAMES: usize = 22_050;

fn tone(i: usize) -> f32 {
    0.5 * (TAU * 440.0 * i as f32 / RATE as f32).sin()
}

fn converted(name: &str) -> DecodedAudio {
    let dest = crate::test_temp::dir().join(name);
    import_mp3_bytes(std::fs::read(FIXTURE).unwrap(), &dest).expect("the MP3 converts");
    decode_file(dest.to_str().unwrap()).expect("the WAV decodes")
}

/// Without the trim the decode is 22050 + encoder delay (~1105) + padding frames.
#[test]
fn an_mp3_decodes_to_exactly_the_frames_that_were_encoded() {
    let audio = decode_file(FIXTURE).unwrap();
    assert_eq!((audio.channels, audio.sample_rate), (1, RATE));
    assert_eq!(audio.frames(), FRAMES);
}

#[test]
fn a_converted_mp3_is_a_wav_of_the_same_length_rate_and_signal() {
    let audio = converted("rusty_import_tone.wav");
    assert_eq!((audio.channels, audio.sample_rate), (1, RATE));
    assert_eq!(audio.frames(), FRAMES);
    // Aligned with the source sample for sample: the delay is gone, not shifted.
    let worst = (0..FRAMES)
        .map(|i| (audio.samples[i] - tone(i)).abs())
        .fold(0.0, f32::max);
    assert!(worst < 0.05, "worst deviation from the source {worst}");
}

/// Looping the converted clip: the step from its last sample back to its first is an
/// ordinary step of the tone, and neither end is padded with silence.
#[test]
fn a_converted_loop_has_no_silence_at_the_seam() {
    let audio = converted("rusty_import_loop.wav");
    let s = &audio.samples;
    let max_step = (0.5 * TAU * 440.0 / RATE as f32) + 0.02;
    let seam = (s[0] - s[s.len() - 1]).abs();
    assert!(seam < max_step, "seam step {seam}");
    let rms = |w: &[f32]| (w.iter().map(|x| x * x).sum::<f32>() / w.len() as f32).sqrt();
    // One period (≈100 samples) of the tone has RMS 0.5/√2 ≈ 0.35.
    assert!(rms(&s[..100]) > 0.3, "head is silent");
    assert!(rms(&s[s.len() - 100..]) > 0.3, "tail is silent");
}

#[test]
fn the_wav_writer_keeps_stereo_channels_apart() {
    let samples: Vec<f32> = (0..200).flat_map(|i| [tone(i), -0.25]).collect();
    let audio = decode_bytes(wav::encode(2, 22_050, &samples)).unwrap();
    assert_eq!(
        (audio.channels, audio.sample_rate, audio.frames()),
        (2, 22_050, 200)
    );
    for (got, want) in audio.samples.iter().zip(&samples) {
        assert!((got - want).abs() < 1e-4, "{got} vs {want}");
    }
}

fn project(name: &str) -> std::path::PathBuf {
    let root = crate::test_temp::dir().join(name);
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("audio/sfx")).unwrap();
    root
}

#[test]
fn refresh_converts_dropped_mp3s_and_removes_them() {
    let root = project("rusty_refresh_converts");
    let mp3 = root.join("audio/sfx/Shot.MP3");
    std::fs::copy(FIXTURE, &mp3).unwrap();
    let report = refresh(&root);
    let wav = root.join("audio/sfx/Shot.wav");
    assert_eq!(report.converted, vec![(mp3.clone(), wav.clone())]);
    assert!(report.skipped.is_empty());
    assert!(!mp3.exists() && wav.exists());
    // A scene that still names the MP3 plays the WAV that replaced it.
    assert_eq!(
        resolve_clip_path(mp3.to_str().unwrap()),
        wav.to_str().unwrap()
    );
    assert!(refresh(&root) == Refresh::default(), "nothing left to do");
}

#[test]
fn refresh_never_overwrites_a_wav_and_retries_an_mp3_that_fails() {
    let root = project("rusty_refresh_skips");
    let (kept, kept_wav) = (root.join("audio/keep.mp3"), root.join("audio/keep.wav"));
    std::fs::copy(FIXTURE, &kept).unwrap();
    std::fs::write(&kept_wav, b"somebody's file").unwrap();
    let broken = root.join("audio/sfx/half_copied.mp3");
    std::fs::write(&broken, b"not audio yet").unwrap();
    let report = refresh(&root);
    assert!(report.converted.is_empty());
    let skipped: Vec<&Path> = report.skipped.iter().map(|(p, _)| p.as_path()).collect();
    assert_eq!(skipped, vec![kept.as_path(), broken.as_path()]);
    assert_eq!(std::fs::read(&kept_wav).unwrap(), b"somebody's file");
    assert!(
        kept.exists() && broken.exists(),
        "left for the runtime fallback"
    );
    // An MP3 still on disk is itself.
    assert_eq!(
        resolve_clip_path(kept.to_str().unwrap()),
        kept.to_str().unwrap()
    );
}
