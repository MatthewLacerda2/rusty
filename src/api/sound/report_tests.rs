//! Tests for the bake report's over-time, spectral and per-track rows, and
//! `Sound.Diff` (#379): known signals land on the rows they must.

use mlua::{Lua, Table};

use super::bake_tests::{lua_with_sound, tmp, IMPACT_PATCH};
use super::level_tests::write_wav;

fn eval<'lua>(lua: &'lua Lua, code: &str) -> Table<'lua> {
    lua.load(code).eval().expect("the verb runs")
}

fn num(row: &Table, key: &str) -> f64 {
    row.get::<_, f64>(key).unwrap()
}

fn row<'lua>(rows: &Table<'lua>, at: i64) -> Table<'lua> {
    rows.get::<_, Table>(at).unwrap()
}

/// Mono samples of a sine at `hz` and linear `amplitude`, `seconds` long.
fn sine(hz: f64, amplitude: f64, seconds: f64) -> Vec<i16> {
    (0..(44_100.0 * seconds) as usize)
        .map(|i| {
            let phase = i as f64 * hz * std::f64::consts::TAU / 44_100.0;
            (32_767.0 * amplitude * phase.sin()) as i16
        })
        .collect()
}

#[test]
fn halves_a_known_level_apart_report_it_on_their_own_rows() {
    let mut samples = sine(1_000.0, 0.25, 8.0);
    samples.extend(sine(1_000.0, 0.5, 8.0));
    let path = write_wav("rusty_report_halves.wav", 1, &samples);
    let lua = lua_with_sound();
    let sections: Table = eval(&lua, &format!("return Sound.Level(\"{path}\")"))
        .get("sections")
        .unwrap();
    assert_eq!(sections.raw_len(), 2, "an 8-second grid over 16 seconds");
    let (first, second) = (row(&sections, 1), row(&sections, 2));
    let step = num(&second, "mean") - num(&first, "mean");
    assert!(
        (step - 6.02).abs() < 0.05,
        "doubling is +6.02 dB, not {step}"
    );
    assert!((num(&second, "from") - 8.0).abs() < 1e-6);
    assert!(second.get::<_, Option<String>>("label").unwrap().is_none());
}

#[test]
fn a_pure_low_tone_puts_its_energy_in_the_low_band_only() {
    let path = write_wav("rusty_report_low.wav", 1, &sine(60.0, 0.5, 2.0));
    let lua = lua_with_sound();
    let bands: Table = eval(&lua, &format!("return Sound.Level(\"{path}\")"))
        .get("bands")
        .unwrap();
    let low: u32 = bands.get("low").unwrap();
    let high: u32 = bands.get("high").unwrap();
    assert!(low >= 95, "a 60 Hz tone is low, not {low}%");
    assert_eq!(high, 0);
}

#[test]
fn a_clip_diffed_with_itself_moves_nowhere() {
    let path = write_wav("rusty_report_self.wav", 1, &sine(440.0, 0.5, 1.0));
    let lua = lua_with_sound();
    let diff = eval(&lua, &format!("return Sound.Diff(\"{path}\", \"{path}\")"));
    assert!(diff.get::<_, bool>("same").unwrap());
    assert_eq!(num(&diff, "mean"), 0.0);
    let bands: Table = diff.get("bands").unwrap();
    assert_eq!(num(&bands, "mid"), 0.0);
}

#[test]
fn a_diff_says_how_much_quieter_the_first_clip_is() {
    let quiet = write_wav("rusty_report_quiet.wav", 1, &sine(440.0, 0.25, 1.0));
    let loud = write_wav("rusty_report_loud.wav", 1, &sine(440.0, 0.5, 1.0));
    let lua = lua_with_sound();
    let diff = eval(&lua, &format!("return Sound.Diff(\"{quiet}\", \"{loud}\")"));
    assert!((num(&diff, "mean") + 6.02).abs() < 0.05);
    assert!(!diff.get::<_, bool>("same").unwrap());
}

#[test]
fn a_one_shot_has_no_section_or_track_rows() {
    let lua = lua_with_sound();
    let out = tmp("rusty_report_oneshot.wav");
    let level = eval(
        &lua,
        &format!("local _, l = Sound.Bake({IMPACT_PATCH}, \"C2\", \"{out}\") return l"),
    );
    let sections: Table = level.get("sections").unwrap();
    let tracks: Table = level.get("tracks").unwrap();
    assert_eq!((sections.raw_len(), tracks.raw_len()), (0, 0));
    std::fs::remove_file(out).ok();
}

#[test]
fn a_song_reports_its_patterns_and_a_silent_track_as_silent() {
    let lua = lua_with_sound();
    let out = tmp("rusty_report_song.wav");
    let level = eval(
        &lua,
        &format!(
            r#"local _, l = Sound.BakeSong({{
                bpm = 120, seed = 3,
                tracks = {{ {{ name = "hit", patch = {IMPACT_PATCH}, gain = 0.3 }},
                            {{ name = "mute", patch = {IMPACT_PATCH} }} }},
                patterns = {{
                    intro = {{ beats = 4, notes = {{ {{ track = "hit", note = "C2", start = 0, dur = 0.5 }} }} }},
                    outro = {{ beats = 4, notes = {{ {{ track = "hit", note = "C2", start = 0, dur = 0.5 }} }} }},
                }},
                arrangement = {{ "intro", "outro" }},
            }}, "{out}") return l"#
        ),
    );
    let sections: Table = level.get("sections").unwrap();
    let label: String = row(&sections, 1).get("label").unwrap();
    assert_eq!(label, "intro", "rows are the arrangement's sections");
    let tracks: Table = level.get("tracks").unwrap();
    let (hit, mute) = (row(&tracks, 1), row(&tracks, 2));
    assert_eq!(hit.get::<_, String>("name").unwrap(), "hit");
    assert!(
        mute.get::<_, bool>("silent").unwrap(),
        "a track with no notes"
    );
    let whole = num(&level, "mean");
    assert!(
        (num(&hit, "mean") - whole).abs() < 0.5,
        "the one live track is the mix"
    );
    std::fs::remove_file(out).ok();
}
