//! Unison: one oscillator written down, several sounding.
//!
//! What the document says, what it refuses, and what the widened entry does to
//! the rendered note — as against `core::osc::unison`, which is where the
//! three per-voice numbers themselves are pinned down.

use crate::common::{minimal, opts, osc, peak, render};
use scorsese_zimmer::patch::{MAX_VOICES, Osc, Source, Wave};
use scorsese_zimmer::{Patch, SynthError};

/// A stack of one oscillator, widened.
fn stack(voices: usize, spread: f32) -> Patch {
    minimal(Source::OscStack {
        oscs: vec![Osc {
            voices,
            spread,
            ..osc(Wave::Saw, 0.0, 0)
        }],
    })
}

/// The default is one voice, and one voice is written into no document — so a
/// recipe from before unison existed keeps its bytes and its cached bake.
#[test]
fn a_recipe_that_says_nothing_gets_the_single_voice_it_always_did() {
    let json = stack(1, 12.0).to_json().expect("serialise");
    assert!(
        !json.contains("voices"),
        "a lone voice was written into {json}"
    );
    assert!(!json.contains("spread"), "the default spread is in {json}");
    let bare = Patch::from_json(
        r#"{"source":{"kind":"osc_stack","oscs":[{"wave":"saw"}]},
            "amp":{"a":0.005,"d":0.0,"s":1.0,"r":0.05}}"#,
    )
    .expect("a recipe written before unison existed still parses");
    assert_eq!(bare.source, stack(1, 12.0).source);
}

#[test]
fn a_widened_oscillator_round_trips_as_written() {
    let patch = stack(MAX_VOICES, 24.0);
    let json = patch.to_json().expect("serialise");
    assert!(json.contains("\"voices\""), "{json}");
    assert!(json.contains("\"spread\""), "{json}");
    assert_eq!(Patch::from_json(&json).expect("deserialise"), patch);
}

/// Both ends of the range are refused rather than quietly rendered: no voices
/// is an oscillator asked for silence without saying so, and past the cap is
/// arithmetic for copies the ear cannot separate.
#[test]
fn a_voice_count_outside_the_range_is_refused() {
    for count in [0, MAX_VOICES + 1, 64] {
        assert_eq!(
            stack(count, 12.0).validate(),
            Err(SynthError::BadVoiceCount {
                found: count,
                limit: MAX_VOICES,
            }),
            "{count} voices"
        );
    }
    for count in 1..=MAX_VOICES {
        stack(count, 12.0).validate().expect("a legal count");
    }
}

/// The refusal carries the number, so an agent repairing a recipe unattended
/// fixes it from the error alone.
#[test]
fn the_refusal_names_the_count_and_the_cap() {
    let message = stack(12, 12.0).validate().expect_err("refused").to_string();
    assert!(message.contains("12"), "{message}");
    assert!(message.contains(&MAX_VOICES.to_string()), "{message}");
}

/// The whole point of the field: one entry is a supersaw, and the four-slot
/// stack is still free for the sub-oscillator underneath it.
#[test]
fn seven_voices_cost_one_stack_slot() {
    let fat = minimal(Source::OscStack {
        oscs: vec![
            Osc {
                voices: MAX_VOICES,
                spread: 25.0,
                ..osc(Wave::Saw, 0.0, 0)
            },
            osc(Wave::Sine, 0.0, -1),
        ],
    });
    fat.validate()
        .expect("two entries is well inside the stack");
    let buf = render(&fat, 57.0, &opts(0.5));
    assert!(buf.iter().all(|s| s.is_finite()));
    assert!(peak(&buf) > 0.2, "the supersaw is inaudible");
}

/// Widening an entry thickens it and does not turn it up, through the whole
/// signal path rather than only inside the stack.
#[test]
fn unison_does_not_raise_the_level_of_a_note() {
    let one = peak(&render(&stack(1, 20.0), 57.0, &opts(0.5)));
    for voices in 2..=MAX_VOICES {
        let many = peak(&render(&stack(voices, 20.0), 57.0, &opts(0.5)));
        assert!(
            many <= one + 1e-3,
            "{voices} voices peaked at {many} over {one}"
        );
        assert!(
            many > 0.3 * one,
            "{voices} voices all but cancelled ({many})"
        );
    }
}

/// And it is genuinely a different sound rather than the same one quieter: the
/// copies are detuned, so they beat.
#[test]
fn a_widened_oscillator_is_not_the_narrow_one_over_again() {
    let narrow = render(&stack(1, 20.0), 57.0, &opts(0.3));
    let wide = render(&stack(5, 20.0), 57.0, &opts(0.3));
    assert_ne!(narrow, wide);
    // A spread of nothing is still five voices, and still not the one — they
    // start in five different places in the cycle.
    assert_ne!(render(&stack(5, 0.0), 57.0, &opts(0.3)), narrow);
}
