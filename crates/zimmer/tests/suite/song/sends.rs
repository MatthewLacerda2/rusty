//! How much of a part reaches the song's room: a track's `send`.

use crate::common::peak;
use crate::common::songs::song;
use scorsese_zimmer::patch::Fx;
use scorsese_zimmer::render_song;
use scorsese_zimmer::song::InlineOnly;
use scorsese_zimmer::song::{Song, Track};

/// The fixture song in a room, its one track sending `send` into it.
fn in_a_room(send: f32) -> Song {
    let mut song = song();
    song.fx = vec![Fx::Reverb {
        size: 0.8,
        damp: 0.3,
        mix: 0.3,
        predelay: 0.0,
    }];
    song.tracks[0].send = send;
    song
}

/// Everything after the last note has finished: past 1.9 s, at 44.1 kHz,
/// interleaved stereo. The fixture's last note stops sounding at 1.8 s.
fn after_the_notes(song: &Song) -> Vec<f32> {
    let mix = render_song(song, &InlineOnly).expect("the song renders");
    mix[(1.9 * 44_100.0) as usize * 2..].to_vec()
}

/// The point of the field: a part that sends nothing is dry in a wet song —
/// once its notes stop, nothing of it is left ringing.
#[test]
fn a_track_that_sends_nothing_leaves_no_tail() {
    let ringing = peak(&after_the_notes(&in_a_room(1.0)));
    assert!(ringing > 1e-3, "the room rang: {ringing}");
    let dry = peak(&after_the_notes(&in_a_room(0.0)));
    assert_eq!(dry, 0.0, "a dry part left a tail");
    let half = peak(&after_the_notes(&in_a_room(0.5)));
    assert!(half > 1e-4 && half < ringing, "{half} against {ringing}");
}

/// A track with a chain of its own is summed through a bus, and the send is
/// applied there too.
#[test]
fn a_bussed_track_sends_what_it_is_told() {
    let mut song = in_a_room(0.0);
    song.tracks[0].fx = vec![Fx::Saturate {
        drive: 1.2,
        mix: 0.5,
    }];
    assert_eq!(peak(&after_the_notes(&song)), 0.0);
}

/// A send only means something to a room: on a song whose chain has none, a
/// track that sends nothing renders the samples one that sends everything
/// does.
#[test]
fn without_a_room_a_send_changes_nothing() {
    let mut glued = in_a_room(0.0);
    glued.fx = vec![Fx::Saturate {
        drive: 1.5,
        mix: 0.5,
    }];
    let mut whole = glued.clone();
    whole.tracks[0].send = 1.0;
    let render = |song: &Song| render_song(song, &InlineOnly).expect("renders");
    assert_eq!(render(&glued), render(&whole));
}

/// The field is absent from a document until it is changed, so every song
/// written before it hashes the bytes it always did — and a written one is
/// read back as written.
#[test]
fn a_full_send_is_never_written_and_a_partial_one_round_trips() {
    let written = serde_json::to_string(&song()).expect("serialises");
    assert!(!written.contains("\"send\""), "{written}");
    let partial = serde_json::to_string(&in_a_room(0.25)).expect("serialises");
    let read: Song = serde_json::from_str(&partial).expect("reads back");
    assert_eq!(read.tracks[0].send, 0.25);
    let bare: Track =
        serde_json::from_str(r#"{ "name": "t", "patch": "kick" }"#).expect("a track with no send");
    assert_eq!(bare.send, 1.0, "absent is everything, as before");
}
