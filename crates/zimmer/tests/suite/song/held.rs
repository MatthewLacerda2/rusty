//! A note held longer than one note may render to (#1093): refused, and named
//! by where the song writes it, rather than cut off at the ceiling with a
//! click at sixty seconds.

use crate::common::songs::{song, verse, voice};
use scorsese_zimmer::song::{InlineOnly, render_song};
use scorsese_zimmer::{NoteAt, SynthError};

#[test]
fn a_note_held_past_the_ceiling_is_refused_where_the_song_writes_it() {
    let mut song = song();
    // 140 beats at 120 bpm is a 70 s pad note.
    voice(verse(&mut song), 1).dur = 140.0;
    let refused = render_song(&song, &InlineOnly).expect_err("refused, not clamped");
    let SynthError::NoteTooLong {
        at, held, limit, ..
    } = &refused
    else {
        panic!("refused for the wrong reason: {refused:?}")
    };
    assert_eq!(
        at.as_ref(),
        Some(&NoteAt {
            pattern: "verse".to_owned(),
            track: "bass".to_owned(),
            start: 1.0,
        })
    );
    assert_eq!((*held, *limit), (70.0, 60.0));
    let said = refused.to_string();
    assert!(
        said.contains("`bass`") && said.contains("`verse`") && said.contains("beat 1"),
        "{said}"
    );
}
