//! Which tracks get a row: a silent track still does, a lone track or a patch bake does not.

use crate::common::songs::{note, played};

use super::{bake, duet, row, tone};

#[test]
fn a_track_that_never_plays_is_reported_silent() {
    let mut absent = duet();
    absent.patterns.get_mut("verse").expect("verse").notes =
        played(vec![note("sub", "E1", 0.0, 2.0)]);
    let tracks = bake(&absent).tracks;
    assert_eq!(tracks.len(), 2, "a silent track is still a track");
    assert!(row(&tracks, "bell").level.loudness.is_silent());
}

#[test]
fn a_song_of_one_track_gets_no_rows() {
    let mut alone = duet();
    alone.tracks.truncate(1);
    alone.patterns.get_mut("verse").expect("verse").notes =
        played(vec![note("sub", "E1", 0.0, 2.0)]);
    assert!(bake(&alone).tracks.is_empty());
}

#[test]
fn a_patch_bake_has_no_rows() {
    let opts = crate::common::opts(0.2);
    let bake = rusty_zimmer::bake_note(&tone(), 60.0, &opts).expect("the patch bakes");
    assert!(bake.tracks.is_empty());
}
