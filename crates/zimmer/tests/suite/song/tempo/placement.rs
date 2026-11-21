//! Where things land once the tempo moves: notes, gates, sections, windows
//! and faders, all off the one map.

use scorsese_zimmer::song::{Automation, Easing, Param, Point, TempoChange};
use scorsese_zimmer::{Excerpt, Span, Window, render_excerpt};

use super::setup::{RATE, close, ends, frame, left, onset, slowing};
use crate::common::songs::{blip, song};
use scorsese_zimmer::song::{InlineOnly, PatchRef, Song};

#[test]
fn a_note_after_a_jump_lands_at_the_new_tempo() {
    let buf = left(&slowing());
    // Beat 2 is one second in, at 120; beat 3 one second after, at 60. An
    // attack starts from silence, so the first sound is the frame after.
    assert_eq!(onset(&buf, 0.9), frame(1.0) + 1);
    assert_eq!(onset(&buf, 1.9), frame(2.0) + 1);
}

#[test]
fn a_gate_is_held_for_as_long_as_its_beats_last_where_it_is_played() {
    // The fixture's blip, held rather than decaying, so the note sounds for
    // exactly its gate and then its 50 ms release.
    let held = |song: Song| {
        let mut patch = blip();
        patch.amp.s = 1.0;
        let mut song = song;
        song.tracks[0].patch = PatchRef::Inline(Box::new(patch));
        left(&song)
    };
    // The note on beat 2 is half a beat long: a quarter of a second at 120,
    // half a second at 60. At 120 the next note is on at 1.5 s, so the search
    // stops short of it; at 60 it is on at 2.
    let sounding = |buf: &[f32], until: f32| {
        buf[frame(1.0)..frame(until)]
            .iter()
            .rposition(|sample| *sample != 0.0)
            .expect("the note sounds") as f32
            / RATE
    };
    let (plain, slowed) = (held(song()), held(slowing()));
    assert!((sounding(&plain, 1.45) - 0.3).abs() < 0.01);
    assert!((sounding(&slowed, 1.9) - 0.55).abs() < 0.01);
}

#[test]
fn sections_end_where_the_map_puts_their_last_beat() {
    assert!(close(&ends(&slowing()), &[1.0, 3.0]));
}

#[test]
fn sections_under_a_ramp_follow_the_integral() {
    // 120 rising evenly to 240 over eight beats: 15 bpm a beat, so beat b
    // falls at (60 / 15) · ln(1 + 15b / 120) = 4 · ln(1 + b / 8).
    let mut song = song();
    song.arrangement = vec!["verse".into(); 4];
    song.tempo = vec![TempoChange::ramp(8.0, 240.0)];
    let want: Vec<f64> = [2.0, 4.0, 6.0, 8.0]
        .iter()
        .map(|beats: &f64| 4.0 * (1.0 + beats / 8.0).ln())
        .collect();
    assert!(close(&ends(&song), &want), "{:?}", ends(&song));
}

#[test]
fn a_window_in_beats_opens_and_closes_on_the_map() {
    let window = Excerpt::of(Window::beats(
        Span::new(2.0, Some(4.0)).expect("a legal span"),
    ));
    let part = render_excerpt(&slowing(), &InlineOnly, &window).expect("renders");
    // Beats 2 to 4 are the second verse: one second in, two seconds long.
    assert_eq!(part.len() / 2, frame(2.0));
}

#[test]
fn a_fader_reads_its_curve_at_the_beat_the_map_says_it_is() {
    // Silent everywhere but beats 3 to 3.5. The note on beat 3 is two seconds
    // in; a fader reading the written 120 bpm would think that beat 4, and
    // silence it.
    let point = |beat, value| Point {
        beat,
        value,
        easing: Easing::Hold,
    };
    let song = Song {
        automation: vec![Automation {
            track: "bass".to_owned(),
            param: Param::Gain,
            points: vec![point(0.0, 0.0), point(3.0, 1.0), point(3.5, 0.0)],
        }],
        ..slowing()
    };
    let buf = left(&song);
    assert!(buf[..frame(1.9)].iter().all(|sample| *sample == 0.0));
    assert!(
        buf[frame(2.0)..frame(2.4)]
            .iter()
            .any(|sample| *sample != 0.0)
    );
}
