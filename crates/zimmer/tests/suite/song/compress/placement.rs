//! Where a sidechain may listen from: another track of this song, from a track's own chain.

use crate::common::saw_patch;
use rusty_zimmer::song::Song;
use rusty_zimmer::SynthError;

use super::{compressor, under_a_silent_kick};

#[test]
fn a_sidechain_has_to_name_another_track_of_this_song() {
    let missing = under_a_silent_kick(vec![compressor(Some("nobody"))]);
    assert_eq!(
        missing.validate(),
        Err(SynthError::UnknownSidechain {
            track: "pad".to_owned(),
            key: "nobody".to_owned(),
        })
    );
    let itself = under_a_silent_kick(vec![compressor(Some("pad"))]);
    assert_eq!(
        itself.validate(),
        Err(SynthError::SelfSidechain {
            track: "pad".to_owned(),
        })
    );
}

#[test]
fn only_a_track_chain_sits_where_one_part_can_listen_to_another() {
    let misplaced = |place| {
        Err(SynthError::MisplacedSidechain {
            place,
            key: "bass".to_owned(),
        })
    };
    let on_the_sum = Song {
        fx: vec![compressor(Some("bass"))],
        ..under_a_silent_kick(Vec::new())
    };
    assert_eq!(on_the_sum.validate(), misplaced("song"));

    let mut instrument = saw_patch();
    instrument.fx = vec![compressor(Some("bass"))];
    assert_eq!(instrument.validate(), misplaced("patch"));
}
