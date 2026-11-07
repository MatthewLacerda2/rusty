//! The issue's bar (#1748): 40 gunshots at once through an SFX group with a
//! compressor stay under 0 dBFS in a capture, held there by Master's default
//! limiter; with the limiter removed the same volley clips. The same volley in a
//! reverb zone, and against each speaker mode: `loud_hall_tests.rs` (#1855).

use super::wav::WavStream;
use super::*;
use crate::audio::device::decode::decode_file;
use crate::audio::mixer::effects::{Fields, Value};
use crate::audio::mixer::EffectChain;
use crate::components::AudioSourceComponent;

/// A stereo gunshot: a full-scale noise burst decaying over 0.3 s.
fn gunshot() -> PathBuf {
    let path = crate::test_temp::dir().join("rusty_effects_gunshot.wav");
    let mut wav = WavStream::create(&path).unwrap();
    let mut seed = 0x2545_f491u32;
    let frames = RATE as usize * 3 / 10;
    let samples: Vec<f32> = (0..frames * 2)
        .map(|i| {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let noise = (seed >> 8) as f32 / (1u32 << 23) as f32 - 1.0;
            noise.signum() * (-(i as f32) / (frames as f32 * 0.3)).exp()
        })
        .collect();
    wav.write(&samples).unwrap();
    wav.finish(RATE).unwrap();
    path
}

/// −0.3 dBFS, the default limiter's ceiling.
pub(super) fn ceiling() -> f32 {
    10f32.powf(-0.3 / 20.0)
}

/// The loudest sample of a capture.
pub(super) fn peak(samples: &[f32]) -> f32 {
    samples.iter().fold(0.0f32, |p, s| p.max(s.abs()))
}

/// A 1.2 s capture of `shots` gunshots at `volume` through SFX, after `setup`.
pub(super) fn capture(
    name: &str,
    shots: u32,
    volume: f32,
    setup: impl FnOnce(&mut AudioMaestro),
) -> Vec<f32> {
    let shot = gunshot();
    let out = crate::test_temp::dir().join(name);
    let mut maestro = AudioMaestro::default();
    setup(&mut maestro);
    maestro.start_capture(&out).unwrap();
    let source = AudioSourceComponent {
        clip: shot.to_string_lossy().into_owned(),
        spatial_blend: 0.0,
        volume,
        output_group: "SFX".to_string(),
        ..Default::default()
    };
    for entity in 0..shots {
        assert!(maestro.play_source(entity, &source, [0.0; 3], 0));
    }
    for _ in 0..60 {
        maestro.advance_output(0.02, false);
    }
    maestro.stop_capture().unwrap();
    let heard = decode_file(&out.to_string_lossy()).unwrap();
    assert!(heard.samples().len() > 2 * RATE as usize, "1.2 s of stereo");
    heard.samples().to_vec()
}

/// 40 gunshots through SFX's compressor, with `master`'s effects on Master.
fn volley(name: &str, master: EffectChain) -> f32 {
    peak(&capture(name, 40, 1.0, |maestro| {
        let n = |k: &str, v: f64| (k.to_string(), Value::Number(v));
        let compressor: Fields = vec![
            ("kind".to_string(), Value::Text("compressor".to_string())),
            n("threshold", -12.0),
            n("ratio", 8.0),
            n("attack", 5.0),
        ];
        let chain = maestro.mixer().parse_effects(&[compressor]).unwrap();
        maestro.set_group_effects("SFX", chain).unwrap();
        maestro.set_group_effects("Master", master).unwrap();
    }))
}

#[test]
fn forty_gunshots_through_a_compressor_and_the_master_limiter_stay_under_full_scale() {
    let peak = volley("rusty_effects_volley.wav", EffectChain::master());
    assert!(peak <= ceiling() * 1.000_001, "peak {peak} past −0.3 dBFS");
    assert!(peak > 0.9, "and the volley is loud: {peak}");
    let unlimited = volley("rusty_effects_volley_raw.wav", EffectChain::default());
    assert!(unlimited > 1.0, "without the limiter it clips: {unlimited}");
}
