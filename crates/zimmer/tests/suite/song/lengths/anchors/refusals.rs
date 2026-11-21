//! What the clock cannot honour, refused before anything is rendered.

use scorsese_zimmer::SynthError;
use scorsese_zimmer::song::{Anchor, Fit, FitMode};

use super::four_sections;

/// The refusal, as the section it names and the words it says.
fn refused(anchors: Vec<Anchor>, fit: Option<Fit>) -> (usize, String) {
    let mut song = four_sections(anchors);
    song.fit = fit;
    match song.validate() {
        Err(error @ SynthError::BadAnchor { section, .. }) => (section, error.to_string()),
        other => panic!("expected an anchor refusal, got {other:?}"),
    }
}

#[test]
fn a_clip_nobody_resolved_is_refused_by_name() {
    let unresolved = Anchor {
        section: 2,
        seconds: None,
        clip: Some("logo".into()),
    };
    let (section, said) = refused(vec![unresolved], None);
    assert_eq!(section, 2);
    assert!(said.contains("clip `logo`"), "{said}");
}

/// Twice as fast is far past a quarter: refused, with the tempo it needed.
#[test]
fn a_stretch_past_the_bound_says_what_tempo_it_needed() {
    let (section, said) = refused(vec![Anchor::at(2, 2.0)], None);
    assert_eq!(section, 2);
    assert!(said.contains("240.0 bpm"), "{said}");
    assert!(said.contains("120.0 bpm written"), "{said}");
}

/// A quarter exactly is within the bound, as it is for a `stretch` fit:
/// three sections written over 6 s, played over 8, are three quarters as fast.
#[test]
fn a_quarter_exactly_is_allowed() {
    let song = four_sections(vec![Anchor::at(3, 8.0)]);
    assert_eq!(song.validate(), Ok(()));
}

/// The end a stretch fit lands is held to the same bound, and says so.
#[test]
fn the_end_of_a_stretch_fit_is_bounded_too() {
    let fit = Some(Fit::lasting(20.0, FitMode::Stretch));
    let (section, said) = refused(vec![Anchor::at(2, 4.0)], fit);
    assert_eq!(section, 4, "the arrangement's length names its end");
    assert!(said.contains("the end `fit` stretches to"), "{said}");
}

/// A stretch fit whose end comes before the last anchor has nowhere to put
/// the sections between them.
#[test]
fn a_stretch_fit_ending_before_the_last_anchor_is_refused() {
    let fit = Some(Fit::lasting(4.0, FitMode::Stretch));
    let (section, said) = refused(vec![Anchor::at(2, 4.0)], fit);
    assert_eq!(section, 2);
    assert!(said.contains("not before the end"), "{said}");
}

#[test]
fn anchors_out_of_order_are_refused() {
    let backwards = refused(vec![Anchor::at(3, 6.0), Anchor::at(2, 4.0)], None);
    assert_eq!(backwards.0, 2);
    let (section, said) = refused(vec![Anchor::at(2, 4.0), Anchor::at(3, 3.9)], None);
    assert_eq!(section, 3);
    assert!(said.contains("not after section 2"), "{said}");
}

#[test]
fn what_no_clock_can_play_is_refused() {
    assert_eq!(refused(vec![Anchor::at(4, 8.0)], None).0, 4, "past the end");
    assert_eq!(
        refused(vec![Anchor::at(0, 1.0)], None).0,
        0,
        "first not at 0"
    );
    assert_eq!(refused(vec![Anchor::at(2, -1.0)], None).0, 2, "negative");
    let (_, said) = refused(vec![Anchor::at(2, f32::INFINITY)], None);
    assert!(said.contains("zero or more seconds"), "{said}");
    let looped = Some(Fit::lasting(8.0, FitMode::Loop));
    assert_eq!(refused(vec![Anchor::at(2, 4.0)], looped).0, 2, "a loop");
}
