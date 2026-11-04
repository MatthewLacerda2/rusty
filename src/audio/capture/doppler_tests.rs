//! Doppler through the mix capture (#1749): a 1 kHz tone flying past the listener
//! at 50 m/s is heard at 343/293 of its pitch coming and 343/393 going, and a
//! still one at exactly its pitch.

use glam::{Quat, Vec3};

use super::wav::WavStream;
use super::*;
use crate::audio::device::decode::decode_file;
use crate::audio::{Listener, MixEnv};
use crate::components::AudioSourceComponent;

const TONE_HZ: f64 = 1_000.0;
const DT: f32 = 0.01;

/// One second of a stereo 1 kHz sine at `RATE`, which loops seamlessly.
fn sine(name: &str) -> PathBuf {
    let path = crate::test_temp::dir().join(name);
    let mut wav = WavStream::create(&path).unwrap();
    let samples: Vec<f32> = (0..RATE)
        .flat_map(|i| {
            let s = (std::f64::consts::TAU * TONE_HZ * f64::from(i) / f64::from(RATE)).sin();
            [s as f32 * 0.5; 2]
        })
        .collect();
    wav.write(&samples).unwrap();
    wav.finish(RATE).unwrap();
    path
}

/// Capture `ticks` ticks of a fully 3D looping tone at `at(t)`, against a listener
/// at the origin; the capture's mono sum and the rate the voice ended at.
fn capture(name: &str, ticks: u32, at: impl Fn(f32) -> Vec3) -> (Vec<f32>, f32) {
    let clip = sine(&format!("{name}.wav"));
    let source = AudioSourceComponent {
        clip: clip.to_string_lossy().into_owned(),
        looping: true,
        spatial_blend: 1.0,
        initial_distance: 1_000.0,
        final_distance: 2_000.0,
        ..Default::default()
    };
    let out = crate::test_temp::dir().join(format!("{name}_out.wav"));
    let mut maestro = AudioMaestro::default();
    maestro.start_capture(&out).unwrap();
    assert!(maestro.play_source(7, &source, at(0.0).to_array(), 0));
    let env = MixEnv {
        listener: Listener::from_transform(Vec3::ZERO, Quat::IDENTITY),
        dt: DT,
        ..MixEnv::default()
    };
    for tick in 1..=ticks {
        let now = at(tick as f32 * DT);
        maestro.apply_mix(&env, |_| Some((source.clone(), now)));
        maestro.advance_output(f64::from(DT), false);
    }
    let rate = maestro.source_doppler(7);
    maestro.stop_capture().unwrap();
    let heard = decode_file(&out.to_string_lossy()).unwrap();
    let mono = heard.samples().chunks(2).map(|f| f[0] + f[1]).collect();
    (mono, rate)
}

/// The tone's frequency over `from..to` seconds of `mono`: rising zero crossings,
/// interpolated, first to last.
fn pitch(mono: &[f32], from: f64, to: f64) -> f64 {
    let at = |t: f64| (t * f64::from(RATE)) as usize;
    let mut crossings = Vec::new();
    for i in at(from)..at(to) {
        let (a, b) = (f64::from(mono[i]), f64::from(mono[i + 1]));
        if a < 0.0 && b >= 0.0 {
            crossings.push(i as f64 + a / (a - b));
        }
    }
    let (first, last) = (crossings[0], crossings[crossings.len() - 1]);
    (crossings.len() - 1) as f64 * f64::from(RATE) / (last - first)
}

#[test]
fn a_tone_flying_past_at_50_ms_rises_coming_and_drops_going() {
    // From 100 m down -X to 100 m up +X, 2 m off the line, over four seconds.
    let path = |t: f32| Vec3::new(-100.0 + 50.0 * t, 0.0, -2.0);
    let (mono, _) = capture("rusty_doppler_pass", 400, path);
    let coming = pitch(&mono, 0.5, 1.5) / TONE_HZ;
    let going = pitch(&mono, 2.5, 3.5) / TONE_HZ;
    let (up, down) = (343.0 / 293.0, 343.0 / 393.0);
    assert!((coming - up).abs() < 0.002, "coming {coming}, want {up}");
    assert!((going - down).abs() < 0.002, "going {going}, want {down}");
}

#[test]
fn a_still_tone_keeps_its_pitch() {
    let (mono, rate) = capture("rusty_doppler_still", 100, |_| Vec3::new(-30.0, 0.0, -2.0));
    assert_eq!(rate, 1.0);
    let heard = pitch(&mono, 0.1, 0.9) / TONE_HZ;
    assert!((heard - 1.0).abs() < 1e-4, "{heard}");
}
