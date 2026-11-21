//! A chord as one entry, held to the notes it replaced.
//!
//! Nearly every assertion here is the claim the arrangement transforms are
//! held to, one level down: **writing a chord must equal writing its voices
//! out by hand.** If it does not, the shorthand has stopped meaning the long
//! form, and a document that reads as `Dm7` is playing something else.

use crate::common::saw_patch;
use crate::common::songs::{blip, note, played, song, verse};
use scorsese_zimmer::song::{
    ArrangementEntry, Chord, Humanize, InlineOnly, Note, PatchRef, PatternEntry, Pitch, Play,
    Voicing,
};
use scorsese_zimmer::{Song, SynthError, render_song};

/// Renders a song whose instruments are all inline.
fn render(song: &Song) -> Vec<f32> {
    render_song(song, &InlineOnly).expect("the song renders")
}

/// One chord on the fixture's `bass` track, filling the first beat.
fn block(name: &str, oct: Option<i32>) -> Chord {
    Chord {
        track: "bass".to_owned(),
        chord: Voicing::Name(name.to_owned()),
        oct,
        start: 0.0,
        dur: 1.0,
        vel: 0.9,
        articulation: None,
        arp: None,
        div: None,
        gate: None,
        inv: None,
        voicing: None,
    }
}

/// [`block`] as the pattern entry it becomes.
fn chord(name: &str, oct: Option<i32>) -> PatternEntry {
    block(name, oct).into()
}

/// One voice of a chord written out: the same track, beat, length and force.
fn voice(pitch: Pitch) -> Note {
    Note {
        track: "bass".to_owned(),
        note: pitch,
        start: 0.0,
        dur: 1.0,
        vel: 0.9,
        articulation: None,
    }
}

/// The fixture playing `entries` once, quietly enough that the master limiter
/// never acts — so a level comparison stays a level comparison.
fn playing(entries: Vec<PatternEntry>) -> Song {
    let mut one_pass = song();
    one_pass.arrangement = vec!["verse".into()];
    one_pass.tracks[0].patch = PatchRef::Inline(Box::new(saw_patch()));
    one_pass.tracks[0].gain = 0.15;
    verse(&mut one_pass).notes = entries;
    one_pass
}

/// The same bar with the voices written out one at a time, and a note after
/// them — the trailing note is what proves a chord consumed **four** ordinals
/// rather than one, since a note's seed is derived from its ordinal.
fn spelled(names: [&str; 4]) -> Song {
    let mut voices: Vec<PatternEntry> = names
        .iter()
        .map(|name| voice(Pitch::Name((*name).to_owned())).into())
        .collect();
    voices.push(note("bass", "E2", 1.0, 0.5).into());
    playing(voices)
}

/// The chord form of [`spelled`], trailing note and all.
fn chorded(name: &str) -> Song {
    playing(vec![
        chord(name, Some(3)),
        note("bass", "E2", 1.0, 0.5).into(),
    ])
}

#[test]
fn a_chord_equals_writing_its_voices_out_by_hand() {
    // A stochastic instrument on purpose: matching here means every voice took
    // the ordinal it would have taken written out, noise seed and all.
    let noisy = |mut song: Song| {
        song.tracks[0].patch = PatchRef::Inline(Box::new(blip()));
        render(&song)
    };
    assert_eq!(
        noisy(chorded("Dm7")),
        noisy(spelled(["D3", "F3", "A3", "C4"]))
    );
}

/// A name the table does not carry is refused. Guessing at one would produce a
/// chord that is not wrong enough to notice and not right enough to be meant.
#[test]
fn a_name_off_the_table_is_refused_rather_than_guessed() {
    let wrong = chorded("Cmaj13");
    assert!(
        matches!(wrong.validate(), Err(SynthError::UnknownChord { chord }) if chord == "Cmaj13"),
        "an unknown chord name must be refused, and by name"
    );
}

/// A chord transposes as its resulting pitches do — so `Dm7` up a tone is
/// `Em7`, every voice moved together and none of them left behind.
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

/// Expansion happens **before** the performance, so a chord is four notes
/// being played rather than one block being placed.
#[test]
fn each_voice_of_a_chord_is_humanised_on_its_own() {
    // Velocity only, so what a nudge changed is a level per voice and the
    // arithmetic below can see it.
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

    // One nudge for the whole chord would scale every sample by one number.
    // Four nudges cannot, because the voices are at four different pitches.
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

/// The escape hatch, end to end: a voicing no name spells, still one entry
/// with one `start`, one `dur` and one `vel` between its voices.
#[test]
fn a_chord_spelled_as_pitches_plays_those_pitches() {
    let spread = Chord {
        chord: Voicing::Pitches(vec![
            Pitch::Name("D2".to_owned()),
            Pitch::Name("A3".to_owned()),
            Pitch::Midi(60.0),
        ]),
        ..block("unused", None)
    };
    let by_hand = playing(played(vec![
        voice(Pitch::Name("D2".to_owned())),
        voice(Pitch::Name("A3".to_owned())),
        voice(Pitch::Name("C4".to_owned())),
    ]));
    assert_eq!(render(&playing(vec![spread.into()])), render(&by_hand));
}
