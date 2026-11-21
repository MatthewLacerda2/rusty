//! The delay end to end: the flag that makes it a stereo effect, and what a
//! document that never writes it is still worth.

use crate::common::{channel, opts, render_stereo, saw_patch};
use scorsese_zimmer::patch::{Fx, Patch};

/// A saw through one delay, `ping_pong` as given. A saw rather than a noise
/// blip: `noise` is already two uncorrelated signals, so a fixture that
/// arrives wide cannot say what the delay did.
fn delayed(ping_pong: bool) -> Patch {
    Patch {
        fx: vec![Fx::Delay {
            time: 0.08,
            beats: None,
            feedback: 0.45,
            mix: 0.6,
            ping_pong,
        }],
        ..saw_patch()
    }
}

/// How alike the two channels of a rendered note are, `1.0` being identical.
fn correlation(patch: &Patch) -> f32 {
    let buf = render_stereo(patch, 57.0, &opts(1.0));
    let (left, right) = (channel(&buf, 0), channel(&buf, 1));
    let dot: f32 = left.iter().zip(&right).map(|(l, r)| l * r).sum();
    let energy = |c: &[f32]| c.iter().map(|s| s * s).sum::<f32>().sqrt();
    dot / (energy(&left) * energy(&right)).max(f32::MIN_POSITIVE)
}

/// The whole of what the flag buys, and the whole of what it costs when it is
/// absent: a per-channel delay hands back a centred signal centred, and the
/// same delay asked to cross over does not.
#[test]
fn the_flag_is_the_difference_between_a_position_in_time_and_one_in_space() {
    let flat = correlation(&delayed(false));
    assert!((flat - 1.0).abs() < 1e-4, "a slapback is not width: {flat}");
    let walked = correlation(&delayed(true));
    assert!(walked < 0.9, "the sides are still {walked} alike");
}

/// A recipe that never mentions it renders exactly what it always did, sample
/// for sample. The flag defaults to the old behaviour, so a `SYNTH_VERSION`
/// bump is the only reason an existing delay recipe moves — never this field.
#[test]
fn a_delay_that_says_nothing_is_the_delay_it_always_was() {
    let written = render_stereo(&delayed(false), 57.0, &opts(1.0));
    let json = serde_json::to_string(&delayed(false)).expect("serialise");
    let parsed: Patch = serde_json::from_str(&json).expect("deserialise");
    let read_back = render_stereo(&parsed, 57.0, &opts(1.0));
    assert_eq!(written, read_back);
}

/// The default is not written back. A bake is addressed by the hash of the
/// recipe's bytes, so a serialiser that filled this in would invalidate every
/// cached bake in every project the next time anything saved a patch — a cost
/// this crate pays when the audio changes and never otherwise.
#[test]
fn a_defaulted_ping_pong_is_not_written_back() {
    let saved = serde_json::to_string(&delayed(false).fx[0]).expect("serialise");
    assert!(!saved.contains("ping_pong"), "written back: {saved}");
    let saved = serde_json::to_string(&delayed(true).fx[0]).expect("serialise");
    assert!(saved.contains(r#""ping_pong":true"#), "got {saved}");
}

/// And it is read back the way it is written: absent means the per-channel
/// delay every recipe on disk already has.
#[test]
fn a_chain_that_omits_it_parses_as_the_per_channel_delay() {
    let json = r#"[{ "fx": "delay", "time": 0.25, "feedback": 0.35, "mix": 0.2 }]"#;
    let chain: Vec<Fx> = serde_json::from_str(json).expect("parses without the flag");
    assert_eq!(
        chain,
        vec![Fx::Delay {
            time: 0.25,
            beats: None,
            feedback: 0.35,
            mix: 0.2,
            ping_pong: false,
        }]
    );
}
