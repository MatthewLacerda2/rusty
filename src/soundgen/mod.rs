//! src/soundgen — agent-composed procedural sound authoring (#357).
//!
//! The engine can *play* audio but, until this module, could not *generate* any:
//! sound was the last asset family the agent could only import. Here it composes a
//! **patch** — the serde document describing one instrument — and **bakes** a note
//! of it to a mono `.wav` the audio runtime already decodes. A gunshot, an impact,
//! a footstep or a UI blip is one rendered note of a noise / Karplus / FM patch,
//! which is exactly what a Trepang2-caliber shooter consumes by the thousand.
//!
//! This is the audio leg of the #174 authoring layer and follows its spine —
//! document → runner → bake — with the **#272 shape**, not #270's: a patch is a
//! *structured* document under a fixed contract (*playable as a note*: `(pitch,
//! velocity, duration) in → buffer out`), not a free graph. The agent chooses what
//! fills each stage; the stages themselves never move.
//!
//! It is a sibling of [`crate::procgen`] (textures) and [`crate::shadergen`]
//! (shaders), and stands up the spine the song leg builds on: all the DSP subtlety
//! of the layer lives here.
//!
//! **Determinism.** Every stochastic element draws from the seeded integer hash
//! `procgen` established ([`crate::procgen::hash`], via [`core::noise`]) — no
//! `rand`, no wall clock — so the same patch + note + seed bakes a byte-identical
//! WAV, in any process, on any run.
//!
//! Module layout:
//! - [`patch`] — the serde document (`Patch`, `Source`, `Filter`, `Lfo`, `Fx`):
//!   the source of truth, round-trips losslessly.
//! - [`note`] — note names ↔ MIDI ↔ Hz, plus the per-note render options.
//! - [`core`] — the note renderer: source → filter → amp envelope, with the LFO.
//! - [`fx`] — the post-chain (delay, reverb) and the mandatory bake limiter.
//! - [`wav`] — the mono 16-bit PCM writer, the only place `f32` is quantized.
//! - [`bake`] — the one-call front door: render → limit → write.
//! - [`song`] — the song document (tracks, patterns, arrangement) and its mixer,
//!   which renders every note through the stages above and sums them (#358).
#![deny(clippy::unwrap_used)]

pub mod bake;
pub mod core;
pub mod fx;
pub mod note;
pub mod patch;
pub mod song;
pub mod wav;

pub use bake::{bake_note, render_limited};
// `self::` disambiguates our `core` module from the `core` crate.
pub use self::core::SAMPLE_RATE;
pub use note::{midi_to_freq, parse_note, NoteOpts};
pub use patch::Patch;
pub use song::render::{bake_song, render_song};
pub use song::Song;

/// Where baked sounds belong by convention: the gitignored project workspace, the
/// same pattern authored textures and shaders follow, so a bake never lands in the
/// committed engine assets. A caller passes the full path, so this is guidance the
/// agent reads, not a default the code applies.
pub const SOUND_WORKSPACE: &str = "project/assets/sounds";

/// Bake one note of `patch`, naming the note the way a score does (`"C#4"`, or a
/// MIDI number as text). The one entry point that takes the agent's own spelling of
/// a pitch, so the API surface never has to reach into [`note`] itself.
pub fn bake_named_note(
    patch: &Patch,
    note: &str,
    opts: &NoteOpts,
    path: &str,
) -> Result<String, String> {
    bake_note(patch, parse_note(note)?, opts, path)
}

#[cfg(test)]
mod tests {
    use super::patch::{Adsr, Filter, FilterKind, Fx, Osc, Source, Wave};
    use super::*;

    /// A plucked-string patch touching every optional stage — the closest thing to
    /// a "real" patch the spine-level tests need.
    fn string_patch() -> Patch {
        Patch {
            source: Source::OscStack {
                oscs: vec![
                    Osc {
                        wave: Wave::Saw,
                        detune_cents: -7.0,
                        gain: 1.0,
                        octave: 0,
                    },
                    Osc {
                        wave: Wave::Square,
                        detune_cents: 7.0,
                        gain: 0.6,
                        octave: -1,
                    },
                ],
            },
            amp: Adsr {
                a: 0.005,
                d: 0.15,
                s: 0.4,
                r: 0.2,
            },
            filter: Some(Filter {
                kind: FilterKind::Lowpass,
                cutoff: 800.0,
                resonance: 0.3,
                env_amount: 3000.0,
                adsr: Adsr {
                    a: 0.001,
                    d: 0.2,
                    s: 0.1,
                    r: 0.2,
                },
            }),
            lfo: None,
            fx: vec![Fx::Reverb {
                size: 0.5,
                damp: 0.5,
                mix: 0.2,
            }],
        }
    }

    #[test]
    fn patch_round_trip_re_renders_identically() {
        let patch = string_patch();
        let json = patch.to_json().expect("serialize");
        let back = Patch::from_json(&json).expect("deserialize");
        assert_eq!(patch, back);
        let opts = NoteOpts::default();
        let a = render_limited(&patch, 60.0, &opts).expect("render");
        let b = render_limited(&back, 60.0, &opts).expect("re-render");
        assert_eq!(a, b, "a saved patch must re-render sample-for-sample");
    }

    #[test]
    fn a_named_note_bakes_the_same_file_as_its_midi_number() {
        let dir = std::env::temp_dir();
        let named = dir.join("rusty_soundgen_named.wav");
        let numeric = dir.join("rusty_soundgen_numeric.wav");
        let (named, numeric) = (
            named.to_str().expect("utf-8").to_string(),
            numeric.to_str().expect("utf-8").to_string(),
        );
        let opts = NoteOpts::default();
        bake_named_note(&string_patch(), "C#4", &opts, &named).expect("bake by name");
        bake_note(&string_patch(), 61.0, &opts, &numeric).expect("bake by number");
        assert_eq!(
            std::fs::read(&named).expect("named file"),
            std::fs::read(&numeric).expect("numeric file"),
        );
        std::fs::remove_file(named).ok();
        std::fs::remove_file(numeric).ok();
    }

    #[test]
    fn the_note_number_lands_on_the_right_pitch() {
        // A bare sine, so rising zero-crossings *are* the frequency: one octave up
        // must double them, and A4 must come out at exactly 440 of them per second.
        let sine = Patch {
            source: Source::OscStack {
                oscs: vec![Osc {
                    wave: Wave::Sine,
                    detune_cents: 0.0,
                    gain: 1.0,
                    octave: 0,
                }],
            },
            amp: Adsr {
                a: 0.0,
                d: 0.0,
                s: 1.0,
                r: 0.0,
            },
            filter: None,
            lfo: None,
            fx: vec![],
        };
        let opts = NoteOpts {
            duration: 1.0,
            velocity: 1.0,
            seed: 0,
        };
        let hz = |midi: f32| {
            let buf = render_limited(&sine, midi, &opts).expect("render");
            buf[..44_100]
                .windows(2)
                .filter(|w| w[0] <= 0.0 && w[1] > 0.0)
                .count()
        };
        assert_eq!(hz(69.0), 440, "A4 is 440 Hz");
        assert_eq!(hz(57.0), 220, "an octave down halves it");
        assert_eq!(hz(81.0), 880, "an octave up doubles it");
    }

    #[test]
    fn an_unknown_note_name_is_rejected_by_name() {
        let err = bake_named_note(&string_patch(), "H4", &NoteOpts::default(), "x.wav")
            .expect_err("H is not a note");
        assert!(err.contains("H4"), "the message names the offender: {err}");
    }
}
