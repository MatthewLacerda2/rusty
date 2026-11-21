//! What a note and a song do with the LFO's onset, second slot and beats
//! (#1772). `lfo` has the document; `core::lfo` pins the curves.

use crate::common::songs::song;
use crate::common::{opts, render, saw_patch};
use scorsese_zimmer::patch::{Lfo, LfoShape, LfoTarget};
use scorsese_zimmer::song::{InlineOnly, PatchRef};
use scorsese_zimmer::{Patch, render_song};

fn vibrato() -> Lfo {
    Lfo::new(5.0, 0.5, LfoTarget::Pitch)
}

fn with(lfo: Lfo) -> Patch {
    Patch {
        lfo: Some(lfo),
        ..saw_patch()
    }
}

fn on_the_beat() -> Lfo {
    Lfo {
        beats: Some(0.5),
        ..Lfo::new(0.0, 0.8, LfoTarget::Amp)
    }
}

/// Through its delay a vibrato moves nothing: the note is sample for sample
/// the note with no LFO, and after it the two part.
#[test]
fn a_delayed_vibrato_leaves_the_start_of_the_note_alone() {
    let late = with(Lfo {
        delay: 0.2,
        fade: 0.3,
        depth: 1.0,
        ..vibrato()
    });
    let plain = render(&saw_patch(), 60.0, &opts(0.8));
    let moved = render(&late, 60.0, &opts(0.8));
    let start = (0.2 * 44_100.0) as usize;
    assert_eq!(moved[..start], plain[..start]);
    assert_ne!(moved[start + 10_000..], plain[start + 10_000..]);
}

/// `lfo2` is an LFO like `lfo`: one written there renders what it renders in
/// the first slot.
#[test]
fn the_second_slot_moves_the_note_as_the_first_does() {
    let first = render(&with(vibrato()), 60.0, &opts(0.4));
    let second = Patch {
        lfo2: Some(vibrato()),
        ..saw_patch()
    };
    assert_eq!(render(&second, 60.0, &opts(0.4)), first);
}

/// A song plays a patch whose LFO is timed in beats, the same way twice, and
/// the LFO moves it: a square on the amp, read off the song's clock, dips
/// the level for half of every cycle.
#[test]
fn a_song_plays_an_lfo_timed_in_beats() {
    let play = |lfo: Option<Lfo>| {
        let mut song = song();
        let patch = Patch { lfo, ..saw_patch() };
        song.tracks[0].patch = PatchRef::Inline(Box::new(patch));
        render_song(&song, &InlineOnly).expect("the song renders")
    };
    let square = Lfo {
        shape: LfoShape::Square,
        ..on_the_beat()
    };
    let once = play(Some(square));
    assert_eq!(once, play(Some(square)));
    assert_ne!(once, play(None), "the wobble moved nothing");
}
