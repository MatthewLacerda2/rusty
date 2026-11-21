//! Tests for the tempo map: ramps, jumps, landings and passes, beats to seconds and back.

use super::*;

fn accelerando() -> Map {
    Map::new(120.0, &[TempoChange::ramp(16.0, 180.0)], 1.0, 32.0, 1)
}

fn summed(beats: f64) -> f64 {
    let steps = (beats * 100_000.0) as usize;
    let width = beats / steps as f64;
    (0..steps)
        .map(|step| {
            let beat = (step as f64 + 0.5) * width;
            let bpm = if beat < 16.0 {
                120.0 + 60.0 * beat / 16.0
            } else {
                180.0
            };
            60.0 / bpm * width
        })
        .sum()
}

#[test]
fn a_ramp_places_its_beats_where_the_integral_says() {
    let map = accelerando();
    let ramp = 16.0 * (1.5f64).ln();
    assert!((map.seconds(16.0) - ramp).abs() < 1e-9);
    for beats in [0.5, 3.0, 8.0, 15.9, 16.0, 20.0, 31.0] {
        let got = map.seconds(beats);
        assert!((got - summed(beats)).abs() < 1e-6, "beat {beats}: {got}");
    }
    // Past the ramp the tempo holds at 180: a third of a second a beat.
    assert!((map.seconds(22.0) - (ramp + 2.0)).abs() < 1e-9);
}

#[test]
fn a_landing_mid_ramp_carries_on_from_the_tempo_the_ramp_reached() {
    let changes = [TempoChange::jump(4.0, 60.0), TempoChange::ramp(16.0, 120.0)];
    let written = Map::new(120.0, &changes, 1.0, 32.0, 1);
    let anchored = written.anchored(&[(10.0, 9.0)]);
    // Written: 2 s, then a ramp from 60 that is at 90 by beat 10.
    assert!((written.tempo(10.0) - 90.0).abs() < 1e-9);
    assert!(
        (anchored.tempo(10.0) - 90.0).abs() < 1e-9,
        "after the last landing"
    );
    let factor = written.seconds(10.0) / 9.0;
    assert!((anchored.tempo(6.0) - 70.0 * factor).abs() < 1e-9);
}

#[test]
fn the_tempo_on_a_beat_is_the_map_read_there() {
    let map = accelerando();
    assert_eq!(map.tempo(0.0), 120.0);
    assert!((map.tempo(8.0) - 150.0).abs() < 1e-9);
    assert!((map.tempo(4.0) - 135.0).abs() < 1e-9);
    assert_eq!(map.tempo(20.0), 180.0);
    let twice = Map::new(120.0, &[TempoChange::jump(2.0, 60.0)], 1.0, 4.0, 2);
    assert_eq!(twice.tempo(3.0), 60.0);
    assert_eq!(twice.tempo(5.0), 120.0, "the second pass starts again");
}

#[test]
fn seconds_to_beats_undoes_beats_to_seconds() {
    let map = accelerando();
    for beats in [0.0, 1.0, 7.25, 16.0, 16.5, 40.0] {
        assert!((map.beats(map.seconds(beats)) - beats).abs() < 1e-9);
    }
}

#[test]
fn the_slowest_tempo_is_found_at_either_end_of_a_ramp() {
    assert_eq!(accelerando().slowest(), 120.0);
    let ritardando = Map::new(120.0, &[TempoChange::ramp(16.0, 60.0)], 1.0, 32.0, 1);
    assert_eq!(ritardando.slowest(), 60.0);
    let dip = Map::new(
        120.0,
        &[TempoChange::jump(8.0, 90.0), TempoChange::jump(16.0, 140.0)],
        2.0,
        32.0,
        1,
    );
    assert_eq!(dip.slowest(), 180.0);
}

#[test]
fn a_jump_holds_the_old_tempo_up_to_the_beat_it_is_written_on() {
    let map = Map::new(120.0, &[TempoChange::jump(4.0, 60.0)], 1.0, 8.0, 1);
    assert_eq!(map.seconds(4.0), 2.0, "four beats at 120");
    assert_eq!(map.seconds(6.0), 4.0, "then a second a beat");
}

#[test]
fn a_ramp_down_is_the_same_integral_the_other_way() {
    let map = Map::new(120.0, &[TempoChange::ramp(8.0, 60.0)], 1.0, 8.0, 1);
    assert!((map.seconds(8.0) - 8.0 * 2f64.ln()).abs() < 1e-9);
}

#[test]
fn each_pass_plays_the_map_again_and_the_last_runs_on() {
    let map = Map::new(120.0, &[TempoChange::jump(2.0, 60.0)], 1.0, 4.0, 2);
    // One pass: two beats at half a second, two at a second.
    assert_eq!(map.seconds(4.0), 3.0);
    assert_eq!(map.seconds(5.0), 3.5, "the second pass starts back at 120");
    assert_eq!(map.seconds(8.0), 6.0);
    assert_eq!(map.seconds(9.0), 7.0, "past the end, the final tempo holds");
}

#[test]
fn scaling_the_map_scales_every_second_in_it() {
    let written = accelerando();
    let faster = Map::new(120.0, &[TempoChange::ramp(16.0, 180.0)], 1.25, 32.0, 1);
    for beats in [3.0, 16.0, 30.0] {
        assert!((faster.seconds(beats) * 1.25 - written.seconds(beats)).abs() < 1e-9);
    }
}

#[test]
fn landings_are_exact_and_a_ramp_keeps_its_shape_between_them() {
    let written = accelerando();
    let anchored = written.anchored(&[(8.0, 5.0), (24.0, 12.0)]);
    assert!((anchored.seconds(8.0) - 5.0).abs() < 1e-12);
    assert!((anchored.seconds(24.0) - 12.0).abs() < 1e-12);
    let factor = written.seconds(8.0) / 5.0;
    assert!((anchored.opening() - 120.0 * factor).abs() < 1e-9);
    // The second stretch, beats 8 to 24, has its own factor — and the ramp's end at 16 sits
    // inside it, counted from the landing at 8.
    let second = (written.seconds(24.0) - written.seconds(8.0)) / 7.0;
    for beats in [10.0, 16.0, 20.0] {
        let at = 5.0 + (written.seconds(beats) - written.seconds(8.0)) / second;
        assert!((anchored.seconds(beats) - at).abs() < 1e-12, "beat {beats}");
    }
    for beats in [1.0, 4.0, 7.5] {
        let scaled = written.seconds(beats) / factor;
        assert!(
            (anchored.seconds(beats) - scaled).abs() < 1e-12,
            "beat {beats}"
        );
        assert!((anchored.tempo(beats) - written.tempo(beats) * factor).abs() < 1e-9);
    }
    assert!((anchored.seconds(27.0) - 13.0).abs() < 1e-12);
    for half in 0..64 {
        let beats = f64::from(half) / 2.0;
        assert!((anchored.beats(anchored.seconds(beats)) - beats).abs() < 1e-9);
    }
}
