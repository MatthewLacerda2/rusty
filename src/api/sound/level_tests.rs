//! Tests for the level the `Sound` verbs report (#378): known signals measure
//! where they must, and a bake's second return agrees with `Sound.Level` on the
//! file it wrote — one meter, not two.

use mlua::{Lua, Table};

use super::bake_tests::{lua_with_sound, tmp, IMPACT_PATCH};

/// Write interleaved 16-bit `samples` as a 44.1 kHz WAV — the bare RIFF header,
/// so a test can hand `Sound.Level` a signal of exactly known amplitude.
fn write_wav(name: &str, channels: u16, samples: &[i16]) -> String {
    let (rate, data) = (44_100u32, (samples.len() * 2) as u32);
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&channels.to_le_bytes());
    bytes.extend_from_slice(&rate.to_le_bytes());
    bytes.extend_from_slice(&(rate * u32::from(channels) * 2).to_le_bytes());
    bytes.extend_from_slice(&(channels * 2).to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data.to_le_bytes());
    samples
        .iter()
        .for_each(|s| bytes.extend_from_slice(&s.to_le_bytes()));
    let path = tmp(name);
    std::fs::write(&path, bytes).unwrap();
    path
}

fn level_of<'lua>(lua: &'lua Lua, path: &str) -> Table<'lua> {
    lua.load(format!("return Sound.Level(\"{path}\")"))
        .eval()
        .expect("the file measures")
}

fn num(level: &Table, key: &str) -> f64 {
    level.get::<_, f64>(key).unwrap()
}

#[test]
fn a_full_scale_sine_measures_minus_three_mean_and_zero_peak() {
    let sine: Vec<i16> = (0..44_100)
        .map(|i| (32_767.0 * (i as f64 * 440.0 * std::f64::consts::TAU / 44_100.0).sin()) as i16)
        .collect();
    let lua = lua_with_sound();
    let level = level_of(&lua, &write_wav("rusty_level_sine.wav", 1, &sine));
    assert!(
        (num(&level, "mean") - -3.01).abs() < 0.01,
        "{}",
        num(&level, "mean")
    );
    assert!(num(&level, "peak").abs() < 0.01, "{}", num(&level, "peak"));
    assert!((num(&level, "crest") - 3.01).abs() < 0.02);
    assert!((num(&level, "seconds") - 1.0).abs() < 1e-6);
    assert!(!level.get::<_, bool>("silent").unwrap());
}

#[test]
fn a_silent_file_is_reported_silent_with_no_levels() {
    let lua = lua_with_sound();
    let level = level_of(&lua, &write_wav("rusty_level_silence.wav", 2, &[0; 8_820]));
    assert!(level.get::<_, bool>("silent").unwrap());
    assert!(!level.get::<_, bool>("clipping").unwrap());
    for key in ["mean", "peak", "true_peak", "crest"] {
        assert!(level.get::<_, Option<f64>>(key).unwrap().is_none(), "{key}");
    }
}

/// A quarter-rate sine sampled 45° off its crests: every sample sits at full
/// scale, and the waveform between them overshoots by 3 dB — the case sample peak
/// cannot see and `clipping` exists for.
#[test]
fn an_inter_sample_overshoot_reads_hotter_than_its_samples_and_clips() {
    let wave: Vec<i16> = [32_767, 32_767, -32_767, -32_767].repeat(4_410);
    let lua = lua_with_sound();
    let level = level_of(&lua, &write_wav("rusty_level_overshoot.wav", 1, &wave));
    let over = num(&level, "true_peak") - num(&level, "peak");
    assert!(over > 2.5, "true peak only {over} dB over sample peak");
    assert!(level.get::<_, bool>("clipping").unwrap());
}

#[test]
fn a_bake_returns_its_level_and_it_matches_the_file_on_disk() {
    let lua = lua_with_sound();
    let path = tmp("rusty_level_bake.wav");
    lua.globals().set("OUT", path.as_str()).unwrap();
    let (returned, baked): (String, Table) = lua
        .load(format!(
            "return Sound.Bake({IMPACT_PATCH}, \"C2\", OUT, {{ duration = 0.15, seed = 7 }})"
        ))
        .eval()
        .expect("the bake runs");
    assert_eq!(returned, path, "the path stays the first return");
    assert!(!baked.get::<_, bool>("silent").unwrap());
    assert!(
        !baked.get::<_, bool>("clipping").unwrap(),
        "the limiter holds"
    );
    assert!(
        num(&baked, "seconds") > 0.15,
        "the release rings past the gate"
    );

    // Re-measured from disk through the engine's decoder: the same numbers, up to
    // the 16-bit quantisation the WAV adds.
    let measured = level_of(&lua, &path);
    for key in ["mean", "peak", "true_peak", "crest", "seconds"] {
        let gap = (num(&baked, key) - num(&measured, key)).abs();
        assert!(gap < 0.05, "{key} differs by {gap}");
    }
}

#[test]
fn level_of_an_undecodable_path_is_an_error_naming_it() {
    let lua = lua_with_sound();
    let err = lua
        .load("return Sound.Level(\"does/not/exist.wav\")")
        .eval::<Table>()
        .unwrap_err()
        .to_string();
    assert!(err.contains("does/not/exist.wav"), "{err}");
}
