//! A `fit` whose length comes from somewhere else: `to: "clip"`.

use scorsese_zimmer::SynthError;
use scorsese_zimmer::song::{Fit, FitMode, FitTo, InlineOnly};
use scorsese_zimmer::{Song, render_song};

use super::setup::{fitted, render, samples};
use crate::common::songs::song;

fn to_clip(mode: FitMode) -> Song {
    let mut song = song();
    song.fit = Some(Fit {
        seconds: None,
        to: Some(FitTo::Clip),
        mode,
    });
    song
}

#[test]
fn to_clip_reads_and_writes_back_as_written() {
    let fit: Fit = serde_json::from_str(r#"{ "to": "clip", "mode": "stretch" }"#).expect("parses");
    assert_eq!(fit.to, Some(FitTo::Clip));
    assert_eq!(fit.seconds, None);
    assert_eq!(fit.mode, FitMode::Stretch);
    let written = serde_json::to_string(&fit).expect("serialises");
    assert_eq!(
        written, r#"{"to":"clip","mode":"stretch"}"#,
        "no `seconds` invented"
    );
}

#[test]
fn an_unresolved_clip_fit_is_refused_before_anything_renders() {
    let refusal = render_song(&to_clip(FitMode::Loop), &InlineOnly).expect_err("refused");
    assert!(
        matches!(refusal, SynthError::FitLength { .. }),
        "{refusal:?}"
    );
    assert!(refusal.to_string().contains("`to: \"clip\"`"), "{refusal}");
}

#[test]
fn a_fit_needs_exactly_one_length() {
    let mut neither = song();
    neither.fit = Some(Fit {
        seconds: None,
        to: None,
        mode: FitMode::Loop,
    });
    let mut both = to_clip(FitMode::Loop);
    both.fit = both.fit.map(|fit| Fit {
        seconds: Some(4.0),
        ..fit
    });
    for song in [neither, both] {
        assert!(
            matches!(song.validate(), Err(SynthError::FitLength { .. })),
            "{:?}",
            song.fit
        );
    }
}

#[test]
fn a_resolved_clip_fit_is_the_written_one() {
    for mode in [FitMode::Loop, FitMode::Stretch, FitMode::Once] {
        let mut resolved = to_clip(mode);
        resolved.fit = resolved.fit.map(|fit| fit.resolved(4.3));
        assert_eq!(resolved.fit, Some(Fit::lasting(4.3, mode)));
        let rendered = render(&resolved);
        assert_eq!(rendered.len(), samples(4.3), "{mode:?}");
        assert_eq!(rendered, render(&fitted(4.3, mode)), "{mode:?}");
    }
}
