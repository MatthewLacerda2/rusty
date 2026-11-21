//! Writing a layered entry down, and what it is refused for.

use crate::common::songs::song;
use scorsese_zimmer::song::{ArrangementEntry, Layers, Play};
use scorsese_zimmer::{Song, SynthError};

use super::setup::play;

/// What an agent writes: a bare name, and a solo layered over it an octave up.
const SOLO_OVER_GROOVE: &str =
    r#"{ "layers": ["verse", { "pattern": "verse", "transpose": 12 }] }"#;

#[test]
fn a_layered_entry_round_trips_with_each_layer_in_its_own_form() {
    let entry: ArrangementEntry = serde_json::from_str(SOLO_OVER_GROOVE).expect("parses");
    assert_eq!(
        entry,
        Layers {
            layers: vec![
                "verse".into(),
                Play {
                    transpose: Some(12.0),
                    ..play("verse")
                }
                .into(),
            ],
        }
        .into()
    );
    let mut layered = song();
    layered.arrangement = vec![entry, "verse".into()];
    let json = layered.to_json().expect("serialise");
    assert_eq!(Song::from_json(&json).expect("deserialise"), layered);
}

/// A key is refused by name in either long form — and `pattern` beside
/// `layers` is refused rather than one of the two being quietly dropped.
#[test]
fn a_layered_entry_names_the_key_it_does_not_take() {
    for (written, key) in [
        (r#"{ "layers": ["verse"], "pattern": "verse" }"#, "pattern"),
        (r#"{ "layers": ["verse"], "vel_scale": 0.5 }"#, "vel_scale"),
        (
            r#"{ "layers": [{ "pattern": "verse", "transpse": 3 }] }"#,
            "transpse",
        ),
        (r#"{ "layers": [{ "layers": ["verse"] }] }"#, "layers"),
    ] {
        let error = serde_json::from_str::<ArrangementEntry>(written)
            .expect_err("refused")
            .to_string();
        assert!(error.contains(key), "`{written}` was refused as: {error}");
    }
}

/// An empty `layers` is a slot of no length playing nothing — a typo, since a
/// rest is a pattern with no notes and the beats it should last.
#[test]
fn an_entry_with_no_layers_is_refused() {
    let mut empty = song();
    empty.arrangement = vec![Layers { layers: vec![] }.into()];
    assert_eq!(empty.validate(), Err(SynthError::NoLayers));
}

/// Every layer is held to what a whole entry is: a name that matches no
/// pattern, or a transform that is not a number, is refused wherever it sits.
#[test]
fn every_layer_is_checked_like_an_entry() {
    let mut typo = song();
    typo.arrangement = vec![
        Layers {
            layers: vec!["verse".into(), "vrese".into()],
        }
        .into(),
    ];
    assert_eq!(
        typo.validate(),
        Err(SynthError::UnknownPattern {
            pattern: "vrese".to_owned(),
        })
    );

    let mut nonsense = song();
    nonsense.arrangement = vec![
        Layers {
            layers: vec![
                "verse".into(),
                Play {
                    transpose: Some(f32::NAN),
                    ..play("verse")
                }
                .into(),
            ],
        }
        .into(),
    ];
    assert!(matches!(
        nonsense.validate(),
        Err(SynthError::BadTranspose { .. })
    ));
}
