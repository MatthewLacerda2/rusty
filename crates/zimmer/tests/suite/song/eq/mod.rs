//! An EQ where a mix move actually belongs: on a track, or on the sum.

use crate::common::songs::song;
use rusty_zimmer::patch::{EqBand, EqKind, Fx, MAX_EQ_BANDS};
use rusty_zimmer::song::{InlineOnly, Song};
use rusty_zimmer::{render_song, SynthError};

mod report;

fn render(song: &Song) -> Vec<f32> {
    render_song(song, &InlineOnly).expect("the song renders")
}

fn band(kind: EqKind, gain_db: f32) -> EqBand {
    EqBand {
        kind,
        freq: None,
        gain_db,
        q: 1.0,
    }
}

#[test]
fn a_song_eq_changes_the_mix_without_lengthening_it() {
    let dry = render(&song());
    let carved = render(&Song {
        fx: vec![Fx::Eq {
            bands: vec![band(EqKind::Peak, -9.0)],
        }],
        ..song()
    });
    assert_eq!(
        carved.len(),
        dry.len(),
        "a filter neither delays nor repeats, so the piece does not grow"
    );
    assert_ne!(carved, dry, "and it did reach the mix");
}

#[test]
fn a_track_eq_reaches_that_instrument_s_part() {
    let dry = render(&song());
    let mut treated = song();
    treated.tracks[0].fx = vec![Fx::Eq {
        bands: vec![band(EqKind::HighPass, 0.0)],
    }];
    let treated = render(&treated);
    assert_eq!(treated.len(), dry.len());
    assert_ne!(treated, dry);
}

#[test]
fn bands_parked_at_zero_gain_bake_the_identical_mix() {
    let parked = render(&Song {
        fx: vec![Fx::Eq {
            bands: vec![
                band(EqKind::LowShelf, 0.0),
                band(EqKind::Peak, 0.0),
                band(EqKind::HighShelf, 0.0),
            ],
        }],
        ..song()
    });
    assert_eq!(parked, render(&song()));
}

#[test]
fn the_band_cap_holds_in_both_of_the_song_s_chain_locations() {
    let over = || Fx::Eq {
        bands: vec![band(EqKind::Peak, -3.0); MAX_EQ_BANDS + 1],
    };
    let too_many = SynthError::TooManyEqBands {
        found: MAX_EQ_BANDS + 1,
        limit: MAX_EQ_BANDS,
    };
    let on_the_sum = Song {
        fx: vec![over()],
        ..song()
    };
    assert_eq!(on_the_sum.validate(), Err(too_many.clone()));

    let mut on_a_track = song();
    on_a_track.tracks[0].fx = vec![over()];
    assert_eq!(on_a_track.validate(), Err(too_many));
}
