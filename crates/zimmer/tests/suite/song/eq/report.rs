//! A track EQ in the bake's report: the treated track's row moves, the others stay still.

use crate::common::songs::{note, song, verse};
use rusty_zimmer::patch::{Adsr, EqKind, Fx, Osc, Source, Wave};
use rusty_zimmer::song::{InlineOnly, PatchRef, Song, Track};
use rusty_zimmer::Patch;

use super::band;

fn pad() -> Patch {
    Patch {
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
    }
}

fn muddy() -> Song {
    let mut muddy = song();
    muddy.tracks.push(Track {
        name: "pad".to_owned(),
        patch: PatchRef::Inline(Box::new(pad())),
        gain: 0.8,
        pan: 0.0,
        fx: vec![],
        send: 1.0,
        push: 0.0,
        swing: None,
        humanize: None,
    });
    verse(&mut muddy)
        .notes
        .push(note("pad", "E2", 0.0, 2.0).into());
    muddy
}

fn low(song: &Song, track: &str) -> f64 {
    rusty_zimmer::bake_song(song, &InlineOnly)
        .expect("bakes")
        .tracks
        .iter()
        .find(|layer| layer.name == track)
        .and_then(|layer| layer.level.bands)
        .expect("that track played")
        .low
}

#[test]
fn the_report_s_own_rows_show_the_treated_track_moving_and_the_others_still() {
    let muddy = muddy();

    let mut treated = muddy.clone();
    treated.tracks[1].fx = vec![Fx::Eq {
        bands: vec![band(EqKind::HighPass, 0.0)],
    }];

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
