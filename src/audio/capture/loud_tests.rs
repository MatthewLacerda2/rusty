//! The issue's bar (#1748): 40 gunshots at once through an SFX group with a
//! compressor stay under 0 dBFS in a capture, held there by Master's default
//! limiter; with the limiter removed the same volley clips.

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

/// The loudest sample of a capture of 40 gunshots through SFX, with `master`'s
/// effects on Master.
fn volley(name: &str, master: EffectChain) -> f32 {
    let shot = gunshot();
    let out = crate::test_temp::dir().join(name);
    let mut maestro = AudioMaestro::default();
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
    maestro.start_capture(&out).unwrap();
    let source = AudioSourceComponent {
        clip: shot.to_string_lossy().into_owned(),
        spatial_blend: 0.0,
        output_group: "SFX".to_string(),
        ..Default::default()
    };
    for entity in 0..40 {
        assert!(maestro.play_source(entity, &source, [0.0; 3], 0));
    }
    for _ in 0..30 {
        maestro.advance_output(0.02, false);
    }
    maestro.stop_capture().unwrap();
    let heard = decode_file(&out.to_string_lossy()).unwrap();
    assert!(heard.samples().len() > RATE as usize, "0.6 s of stereo");
    heard.samples().iter().fold(0.0f32, |p, s| p.max(s.abs()))
}

#[test]
fn forty_gunshots_through_a_compressor_and_the_master_limiter_stay_under_full_scale() {
    let peak = volley("rusty_effects_volley.wav", EffectChain::master());
    let ceiling = 10f32.powf(-0.3 / 20.0);
    assert!(peak <= ceiling * 1.000_001, "peak {peak} past −0.3 dBFS");
    assert!(peak > 0.9, "and the volley is loud: {peak}");
    let unlimited = volley("rusty_effects_volley_raw.wav", EffectChain::default());
    assert!(unlimited > 1.0, "without the limiter it clips: {unlimited}");
}
