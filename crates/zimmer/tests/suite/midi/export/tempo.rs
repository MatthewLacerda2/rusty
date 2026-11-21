//! The tempo map in the conductor track: jumps as they are, ramps as steps
//! timed to land where the ramp does.

use scorsese_zimmer::midi::export;

use super::read::{Event, read};
use super::song;

/// `(tick, microseconds per beat)` of every tempo event in the file.
fn tempos(json: &str) -> Vec<(u64, u32)> {
    let file = read(&export(&song(&["a"], json), &[]).expect("exports").bytes);
    file.tracks[0]
        .iter()
        .filter_map(|(tick, event)| match event {
            Event::Tempo(micros) => Some((*tick, *micros)),
            _ => None,
        })
        .collect()
}

const BAR: &str = r#""arrangement": ["v"], "patterns": { "v": { "beats": 16, "notes": [] } }"#;

#[test]
fn jumps_are_tempo_events_and_a_restatement_is_not_one() {
    let events = tempos(&format!(
        r#""bpm": 90, "tempo": [{{ "beat": 4, "bpm": 120 }}, {{ "beat": 6, "bpm": 120 }},
           {{ "beat": 8.5, "bpm": 75 }}], {BAR}"#
    ));
    assert_eq!(events, [(0, 666_667), (7_680, 500_000), (16_320, 800_000)]);
}

/// Seconds to `beat` through a ramp from `from` bpm on beat 4 to `to` on beat
/// 8, held at `from` before it and at `to` after — the closed form, worked here rather than
/// borrowed from the renderer, so the test and the code are two derivations.
fn exact(beat: f64, from: f64, to: f64) -> f64 {
    let before = beat.min(4.0) * 60.0 / from;
    let into = (beat - 4.0).clamp(0.0, 4.0);
    let slope = (to - from) / 4.0;
    let after = (beat - 8.0).max(0.0) * 60.0 / to;
    before + 60.0 / slope * (slope * into / from).ln_1p() + after
}

/// Seconds to `beat` as a MIDI player times it: each stretch at the tempo in
/// force.
fn stepped(beat: f64, events: &[(u64, u32)]) -> f64 {
    let tick = beat * 1_920.0;
    let mut seconds = 0.0;
    for (index, &(at, micros)) in events.iter().enumerate() {
        let next = events.get(index + 1).map_or(f64::MAX, |&(t, _)| t as f64);
        let span = tick.min(next) - at as f64;
        if span <= 0.0 {
            break;
        }
        seconds += span / 1_920.0 * f64::from(micros) / 1e6;
    }
    seconds
}

#[test]
fn a_ramp_is_a_step_a_sixteenth_landing_exactly_on_each_sixteenth() {
    // 60 to 180 in one bar: far steeper than music is written, which is the
    // case the error bound in the module doc is stated for. A ramp runs from
    // the point before it, so a point restating 60 holds the first bar still.
    let events = tempos(&format!(
        r#""bpm": 60, "tempo": [{{ "beat": 4, "bpm": 60 }},
           {{ "beat": 8, "bpm": 180, "ramp": true }}], {BAR}"#
    ));
    assert_eq!(
        events.len(),
        1 + 16 + 1,
        "the opening tempo, 16 steps, and the arrival"
    );
    assert_eq!(
        events[1].0,
        4 * 1_920,
        "the first step is where the ramp begins"
    );
    assert_eq!(*events.last().expect("some"), (8 * 1_920, 333_333));

    for sixteenth in 0..=48 {
        let beat = f64::from(sixteenth) / 4.0;
        let drift = (stepped(beat, &events) - exact(beat, 60.0, 180.0)).abs();
        assert!(drift < 20e-6, "{drift} s off on beat {beat}");
    }
    let worst = (0..=48 * 16)
        .map(|n| f64::from(n) / 64.0)
        .map(|beat| (stepped(beat, &events) - exact(beat, 60.0, 180.0)).abs())
        .fold(0.0, f64::max);
    assert!(worst < 0.005, "{worst} s off inside a step");
}

#[test]
fn a_ramp_is_named_in_what_the_file_approximates() {
    let json = format!(r#""bpm": 120, "tempo": [{{ "beat": 8, "bpm": 80, "ramp": true }}], {BAR}"#);
    let exported = export(&song(&["a"], &json), &[]).expect("exports");
    assert!(
        exported
            .left_out
            .iter()
            .any(|line| line.contains("1 tempo ramp(s)")),
        "{:?}",
        exported.left_out
    );
}
