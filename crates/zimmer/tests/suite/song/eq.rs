//! An EQ where a mix move actually belongs: on a track, or on the sum.

use crate::common::songs::{note, song, verse};
use scorsese_zimmer::patch::{Adsr, EqBand, EqKind, Fx, MAX_EQ_BANDS, Osc, Source, Wave};
use scorsese_zimmer::song::{InlineOnly, PatchRef, Song, Track};
use scorsese_zimmer::{Patch, SynthError, render_song};

fn render(song: &Song) -> Vec<f32> {
    render_song(song, &InlineOnly).expect("the song renders")
}

/// One band, spelled out.
fn band(kind: EqKind, gain_db: f32) -> EqBand {
    EqBand {
        kind,
        freq: None,
        gain_db,
        q: 1.0,
    }
}

/// The move the issue is about, at the place a mix decision is made: the
/// report says the low band is swollen, and one line takes it out without
/// taking the instrument out with it.
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

/// The same on a track, which is where a *single* muddy instrument gets
/// treated — the row-per-track half of the bake report is what points here.
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

/// The bypass rule, stated where it matters most: a recipe carrying a shortlist
/// of bands it is still thinking about renders the *identical file* to one
/// carrying none. Not almost-identical — a bake is addressed by a hash, so
/// "within rounding" would be a different asset.
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

/// The loop the whole feature exists to close: the report says a layer is
/// sitting too low, one band treats *that layer*, and the report says so next
/// time. Measured through the per-track rows the report itself prints, so this
/// is the finding and the fix in the same units.
#[test]
fn the_report_s_own_rows_show_the_treated_track_moving_and_the_others_still() {
    // A saw held low: the layer a report calls muddy, and the one thing the
    // noise fixture cannot be, since noise has no pitch to sit too far down.
    let pad = Patch {
        layers: Vec::new(),
        source: Source::OscStack {
            oscs: vec![Osc {
                wave: Wave::Saw,
                detune_cents: 0.0,
                gain: 1.0,
                octave: 0,
                voices: 1,
                spread: 12.0,
                width: 0.0,
                pulse_width: 0.5,
            }],
        },
        amp: Adsr {
            a: 0.05,
            d: 0.1,
            s: 0.9,
            r: 0.2,
            curve: 0.0,
        },
        filter: None,
        pitch_env: None,
        lfo: None,
        lfo2: None,
        fx: vec![],
    };
    let mut muddy = song();
    muddy.tracks.push(Track {
        name: "pad".to_owned(),
        patch: PatchRef::Inline(Box::new(pad)),
        gain: 0.8,
        pan: 0.0,
        fx: vec![],
        send: 1.0,
        push: 0.0,
        swing: None,
        humanize: None,
    });
    // A plain note is one kind of pattern entry, and a chord is the other; the
    // fixture's own `played` says the same thing for a whole list.
    verse(&mut muddy)
        .notes
        .push(note("pad", "E2", 0.0, 2.0).into());

    let mut treated = muddy.clone();
    treated.tracks[1].fx = vec![Fx::Eq {
        bands: vec![band(EqKind::HighPass, 0.0)],
    }];

    let low = |song: &Song, track: &str| {
        scorsese_zimmer::bake_song(song, &InlineOnly)
            .expect("bakes")
            .tracks
            .iter()
            .find(|layer| layer.name == track)
            .and_then(|layer| layer.level.bands)
            .expect("that track played")
            .low
    };
    assert!(
        low(&treated, "pad") < low(&muddy, "pad") - 0.1,
        "the treated layer moved out of the low band: {} to {}",
        low(&muddy, "pad"),
        low(&treated, "pad")
    );
    assert_eq!(
        low(&treated, "bass"),
        low(&muddy, "bass"),
        "and the untreated one did not move at all"
    );
}

/// The cap belongs to the chain, not to where the chain is written — so the
/// song's own and a track's are both checked, and by the same code.
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
