//! Where anchored sections land, and what the rest of the song does.

use scorsese_zimmer::Song;
use scorsese_zimmer::song::{Anchor, Fit, FitMode};

use super::super::setup::{render, samples};
use super::{ends, four_sections};

fn close(got: &[f64], want: &[f64]) {
    assert_eq!(got.len(), want.len(), "{got:?}");
    for (got, want) in got.iter().zip(want) {
        assert!((got - want).abs() < 1e-5, "{got} against {want}: {got:?}");
    }
}

/// Section 2 written at 4 s is pinned at 4.4 and section 3 at 6.2: the first
/// two sections slow to share the extra 0.4 s, the third speeds up to fit its
/// 1.8 s, and the last — after every anchor — plays at the written 120.
#[test]
fn each_anchored_downbeat_lands_on_its_time() {
    let song = four_sections(vec![Anchor::at(2, 4.4), Anchor::at(3, 6.2)]);
    song.validate().expect("within the bound");
    close(&ends(&song), &[2.2, 4.4, 6.2, 8.2]);
}

/// The rendered file follows the clock: an `exact` tail ends on the last
/// beat, which the anchors moved from 8 s to 8.2.
#[test]
fn the_render_is_as_long_as_the_anchored_arrangement() {
    let song = four_sections(vec![Anchor::at(2, 4.4), Anchor::at(3, 6.2)]);
    assert_eq!(render(&song).len(), samples(8.2));
}

/// Under a `stretch` fit the end of the arrangement is one more anchor: the
/// stretch after the last pinned section moves to land it, and nothing past
/// it is looped.
#[test]
fn a_stretch_fit_lands_the_end_after_the_last_anchor() {
    let mut song = four_sections(vec![Anchor::at(2, 4.4)]);
    song.fit = Some(Fit::lasting(8.0, FitMode::Stretch));
    song.validate().expect("within the bound");
    close(&ends(&song), &[2.2, 4.4, 6.2, 8.0]);
    assert_eq!(render(&song).len(), samples(8.0));
}

/// The first section starts the song, so pinning it at zero says nothing new
/// and changes nothing — not a sample.
#[test]
fn an_anchor_on_the_first_section_at_zero_changes_nothing() {
    let song = four_sections(vec![Anchor::at(0, 0.0)]);
    song.validate().expect("valid");
    assert_eq!(render(&song), render(&four_sections(vec![])));
}

/// A `once` fit is not an anchor: the sections land, the song plays out at
/// the written tempo after the last, and the file is cut to the fit.
#[test]
fn a_once_fit_keeps_the_written_tempo_after_the_last_anchor() {
    let mut song = four_sections(vec![Anchor::at(2, 4.4)]);
    song.fit = Some(Fit::lasting(5.0, FitMode::Once));
    song.validate().expect("valid");
    close(&ends(&song), &[2.2, 4.4, 6.4, 8.4]);
    assert_eq!(render(&song).len(), samples(5.0));
}

/// A song that writes its own tempo map keeps its shape inside each
/// stretch: the jump at beat 12 is still there, scaled with the rest.
#[test]
fn a_written_tempo_map_is_scaled_between_anchors() {
    let mut song = four_sections(vec![Anchor::at(3, 5.0)]);
    song.tempo = vec![scorsese_zimmer::song::TempoChange::jump(4.0, 240.0)];
    // Written: 2 s, then three sections at 1 s each — section 3 at 4 s.
    // Pinned at 5 s, the first twelve beats play 4/5 as fast.
    song.validate().expect("within the bound");
    close(&ends(&song), &[2.5, 3.75, 5.0, 6.0]);
}

/// Absent anchors are not written back, so no existing recipe's bytes — or
/// the bake addressed by them — change.
#[test]
fn anchors_read_and_write_back_as_written() {
    let song = four_sections(vec![Anchor {
        section: 2,
        seconds: None,
        clip: Some("logo".into()),
    }]);
    let json = song.to_json().expect("serialises");
    assert!(json.contains(r#""clip": "logo""#), "{json}");
    assert!(!json.contains("seconds"), "{json}");
    assert_eq!(Song::from_json(&json).expect("parses"), song);
    let plain = four_sections(vec![]).to_json().expect("serialises");
    assert!(!plain.contains("anchors"), "{plain}");
}
