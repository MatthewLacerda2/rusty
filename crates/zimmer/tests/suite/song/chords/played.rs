//! A chord through the song's own changes: an arrangement's transpose and the humanizer reach
//! every voice.

use rusty_zimmer::song::{ArrangementEntry, Humanize, Play};
use rusty_zimmer::Song;

use super::{chord, chorded, playing, render, spelled};

#[test]
fn an_arrangement_transposes_every_voice_of_a_chord_together() {
    let alone = |name: &str| playing(vec![chord(name, Some(3))]);
    let mut up = alone("Dm7");
    up.arrangement = vec![ArrangementEntry::from(Play {
        pattern: "verse".to_owned(),
        transpose: Some(2.0),
        transpose_degrees: None,
        vel_scale: None,
        tracks: None,
    })];
    assert_eq!(render(&up), render(&alone("Em7")));
}

#[test]
fn each_voice_of_a_chord_is_humanised_on_its_own() {
    let feel = Humanize {
        timing: 0.0,
        velocity: 0.3,
        timbre: 0.0,
    };
    let scattered = |song: Song| {
        render(&Song {
            humanize: Some(feel),
            ..song
        })
    };
    let straight = render(&chorded("Dm7"));
    let played = scattered(chorded("Dm7"));
    assert_eq!(
        played,
        scattered(spelled(["D3", "F3", "A3", "C4"])),
        "a chord must be humanised as the notes it expands to"
    );

    // One nudge for the whole chord would scale every sample by one number. Four nudges cannot,
    // because the voices are at four different pitches.
    let ratios: Vec<f32> = played
        .iter()
        .zip(&straight)
        .filter(|(_, rigid)| rigid.abs() > 0.05)
        .map(|(voiced, rigid)| voiced / rigid)
        .collect();
    assert!(ratios.len() > 100, "too little signal to compare levels");
    let widest = ratios.iter().fold(f32::MIN, |a, b| a.max(*b))
        - ratios.iter().fold(f32::MAX, |a, b| a.min(*b));
    assert!(
        widest > 0.01,
        "every sample was scaled by the same number — the chord took one nudge as a block"
    );
}
