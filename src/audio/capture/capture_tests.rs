//! A capture (#1579): one at a time, a maestro with no mixer goes offline keeping
//! its live voices, and the file it writes decodes back to the mix.

use super::wav::WavStream;
use super::*;
use crate::audio::device::decode::decode_file;
use crate::components::AudioSourceComponent;

/// A float WAV of `frames` stereo frames at a constant 0.5, written in `name`.
fn tone(name: &str, frames: usize) -> PathBuf {
    let path = crate::test_temp::dir().join(name);
    let mut wav = WavStream::create(&path).unwrap();
    wav.write(&vec![0.5; frames * 2]).unwrap();
    wav.finish(RATE).unwrap();
    path
}

/// A 2D looping speaker playing `clip`.
fn speaker(clip: &Path) -> AudioSourceComponent {
    AudioSourceComponent {
        clip: clip.to_string_lossy().into_owned(),
        looping: true,
        spatial_blend: 0.0,
        ..Default::default()
    }
}

#[test]
fn a_written_capture_decodes_back_to_its_samples() {
    let path = tone("rusty_capture_roundtrip.wav", 480);
    let clip = decode_file(&path.to_string_lossy()).unwrap();
    assert_eq!((clip.channels(), clip.sample_rate()), (2, RATE));
    assert_eq!(clip.samples(), &[0.5; 960][..]);
}

#[test]
fn a_live_voice_is_heard_once_the_maestro_goes_offline() {
    let clip = tone("rusty_capture_hum.wav", 4_800);
    let out = crate::test_temp::dir().join("rusty_capture_hum_out.wav");
    let mut maestro = AudioMaestro::default();
    assert!(maestro.play_source(7, &speaker(&clip), [0.0; 3], 0));
    maestro.start_capture(&out).unwrap();
    assert!(maestro.is_source_playing(7), "the voice survived the swap");
    for _ in 0..6 {
        maestro.advance_output(0.01);
    }
    assert_eq!(maestro.stop_capture().unwrap(), out);
    let heard = decode_file(&out.to_string_lossy()).unwrap();
    assert_eq!(heard.samples().len(), 2 * 2_880, "60 ms at 48 kHz, stereo");
    assert!(
        heard.samples().iter().all(|s| *s > 0.0),
        "the hum plays throughout"
    );
}

#[test]
fn one_capture_runs_at_a_time() {
    let dir = crate::test_temp::dir();
    let mut maestro = AudioMaestro::default();
    assert!(maestro.stop_capture().is_err(), "nothing to stop");
    maestro
        .start_capture(&dir.join("rusty_capture_a.wav"))
        .unwrap();
    let second = maestro.start_capture(&dir.join("rusty_capture_b.wav"));
    assert!(second.unwrap_err().contains("rusty_capture_a.wav"));
    assert!(maestro.capture_path().is_some());
    maestro.stop_capture().unwrap();
    assert!(maestro.capture_path().is_none());
}

#[test]
fn a_silent_capture_is_a_valid_empty_file_at_the_offline_rate() {
    let out = crate::test_temp::dir().join("rusty_capture_empty.wav");
    let mut maestro = AudioMaestro::default();
    maestro.start_capture(&out).unwrap();
    maestro.stop_capture().unwrap();
    assert_eq!(std::fs::read(&out).unwrap(), wav::header(RATE, 0));
}
