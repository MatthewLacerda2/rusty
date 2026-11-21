//! What the page says an arpeggio is, and what it is refused for.

use super::setup::{arped, playing, same};
use scorsese_zimmer::song::{Arp, Chord};
use scorsese_zimmer::{Song, SynthError};

/// The words round-trip, and a block chord writes none of the three fields — a
/// bake is addressed by the bytes of its recipe, so a default written into
/// every chord would miss the cache for every song in every project.
#[test]
fn an_arpeggio_round_trips_and_a_block_chord_says_nothing() {
    let figure = playing(vec![
        arped("Dm7", Some(Arp::UpDown), Some(0.5), Some(0.4)).into(),
    ]);
    let json = figure.to_json().expect("the song serialises");
    assert!(json.contains("\"arp\": \"up_down\""), "{json}");
    assert_eq!(Song::from_json(&json).expect("reads back"), figure);

    let block = playing(vec![arped("Dm7", None, None, None).into()]);
    let json = block.to_json().expect("the song serialises");
    for field in ["\"arp\"", "\"div\"", "\"gate\""] {
        assert!(!json.contains(field), "{field} in {json}");
    }
}

/// The fields an arpeggio needs, and the ones it forbids — each held to the
/// **sentence** it prints, not merely to being refused. Every one of these is
/// repaired by editing a different field, so a refusal that names the wrong one
/// sends the reader to the wrong line, and the reader is usually an agent.
#[test]
fn what_an_arpeggio_is_refused_for_and_what_it_says() {
    let refused = |chord: Chord, says: &str| {
        let refusal = playing(vec![chord.into()]).validate();
        let Err(error) = refusal else {
            panic!("expected a refusal saying {says:?}");
        };
        assert!(matches!(error, SynthError::BadArp { .. }), "{error:?}");
        let said = error.to_string();
        assert!(said.contains(says), "expected {says:?}, got {said:?}");
    };
    let up = |div, gate| arped("Dm7", Some(Arp::Up), div, gate);
    refused(up(None, None), "an `arp` needs a `div`");
    refused(
        arped("Dm7", None, Some(0.5), None),
        "`div` is the step of a",
    );
    refused(arped("Dm7", None, None, Some(0.5)), "a block chord's gate");
    refused(up(Some(8.0), None), "longer than the chord's `dur`");
    refused(up(Some(0.0), None), "`div` is how long one step");
    refused(up(Some(f32::NAN), None), "`div` is how long one step");
    refused(up(Some(0.5), Some(0.0)), "`gate` is how long one note");
    refused(up(Some(0.5), Some(-1.0)), "`gate` is how long one note");
    refused(up(Some(0.0001), None), "more than 4096 notes");
}

/// Both edges of the figure, stated as the last legal step on each side. A
/// boundary nothing exercises is a boundary that can move without anybody
/// noticing, and these two decide whether a one-note figure sounds at all and
/// where the cap actually falls.
#[test]
fn the_edges_of_a_figure_are_legal_and_the_step_past_them_is_not() {
    let one_step = Chord {
        dur: 1.0,
        ..arped("Dm7", Some(Arp::Up), Some(1.0), None)
    };
    same(one_step, "D3", 1.0, 1.0);

    let of_length = |dur| {
        let long = Chord {
            dur,
            ..arped("Dm7", Some(Arp::Up), Some(1.0), None)
        };
        // Fast enough that 4096 beats is under the ten minutes a song may
        // last, so the figure's own cap is the one met.
        let mut song = playing(vec![long.into()]);
        song.bpm = 480.0;
        song.validate()
    };
    assert!(
        of_length(4096.0).is_ok(),
        "4096 notes is the most it sounds"
    );
    assert!(
        matches!(of_length(4097.0), Err(SynthError::BadArp { .. })),
        "4097 is one past the cap"
    );
}
