//! src/soundgen/bake.rs — the one-call front door: patch + note → `.wav` (#357).
//!
//! [`bake_note`] is the verb the API surface drives. It:
//! 1. renders one note of the patch through [`core::render`](crate::soundgen::core::render)
//!    (source → filter → amp → fx),
//! 2. runs the mandatory [`limiter`](crate::soundgen::fx::limiter) — a bake must
//!    never clip, and that is not the agent's decision to make, and
//! 3. writes a mono 16-bit PCM WAV at 44.1 kHz.
//!
//! The returned path is what `AudioSourceComponent.clip` consumes, so a baked
//! gunshot is playable the moment the call returns — no import step, no manifest
//! entry, no editor round-trip.
//!
//! **Determinism:** the whole path is a pure function of `(patch, note, opts.seed)`.
//! Render the same one-shot twice, in any two processes, and the files are
//! byte-identical — the same promise `procgen` makes for textures, and what lets a
//! scene reference a baked asset without shipping it.

use super::core::{self, SAMPLE_RATE};
use super::fx::limiter;
use super::note::NoteOpts;
use super::patch::Patch;
use super::wav;

/// Render one note of `patch` at MIDI pitch `midi` and write it to `path`.
/// Returns the written path.
pub fn bake_note(patch: &Patch, midi: f32, opts: &NoteOpts, path: &str) -> Result<String, String> {
    let samples = render_limited(patch, midi, opts)?;
    // `render_limited` already held the peak under full scale; the writer's own
    // clamp is belt and braces for the quantization rounding.
    wav::write(&samples, SAMPLE_RATE, path)
}

/// Render one note and limit it — the in-memory half of a bake, so callers that
/// want the samples (tests, and the song leg's mixer) need not go through a file.
pub fn render_limited(patch: &Patch, midi: f32, opts: &NoteOpts) -> Result<Vec<f32>, String> {
    let mut samples = core::render(patch, midi, opts)?;
    limiter::apply(&mut samples, core::RATE);
    Ok(samples)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::soundgen::patch::{Adsr, Source};

    /// The simplest audible patch: white noise through the default envelope — a
    /// stand-in for the "hiss" half of every impact sound.
    fn noise_patch() -> Patch {
        Patch {
            source: Source::Noise,
            amp: Adsr {
                a: 0.001,
                d: 0.05,
                s: 0.0,
                r: 0.05,
            },
            filter: None,
            lfo: None,
            fx: vec![],
        }
    }

    fn tmp(name: &str) -> String {
        std::env::temp_dir()
            .join(name)
            .to_str()
            .expect("utf-8 temp path")
            .replace('\\', "/")
    }

    #[test]
    fn a_bake_writes_a_wav_of_the_expected_length() {
        let path = tmp("rusty_soundgen_bake.wav");
        let opts = NoteOpts {
            duration: 0.25,
            velocity: 1.0,
            seed: 3,
        };
        assert_eq!(bake_note(&noise_patch(), 60.0, &opts, &path).unwrap(), path);
        let bytes = std::fs::read(&path).expect("the file exists");
        // 0.25 s gate + 0.05 s release at 44.1 kHz, 2 bytes a sample, 44-byte header.
        let samples = (bytes.len() - 44) / 2;
        assert!(
            (13_000..=13_300).contains(&samples),
            "got {samples} samples for a 0.3 s note"
        );
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn the_same_patch_note_and_seed_bake_identical_bytes() {
        let a = tmp("rusty_soundgen_det_a.wav");
        let b = tmp("rusty_soundgen_det_b.wav");
        let opts = NoteOpts {
            duration: 0.1,
            velocity: 0.9,
            seed: 1234,
        };
        bake_note(&noise_patch(), 64.0, &opts, &a).unwrap();
        bake_note(&noise_patch(), 64.0, &opts, &b).unwrap();
        assert_eq!(
            std::fs::read(&a).unwrap(),
            std::fs::read(&b).unwrap(),
            "same patch + note + seed must bake byte-identical WAV"
        );
        std::fs::remove_file(a).ok();
        std::fs::remove_file(b).ok();
    }

    #[test]
    fn a_different_seed_or_note_changes_the_bytes() {
        let base = NoteOpts {
            duration: 0.1,
            velocity: 0.9,
            seed: 1,
        };
        let reference = render_limited(&noise_patch(), 64.0, &base).unwrap();
        let reseeded = render_limited(&noise_patch(), 64.0, &NoteOpts { seed: 2, ..base }).unwrap();
        assert_ne!(reference, reseeded);
    }

    #[test]
    fn the_limiter_is_not_optional() {
        // A patch loud enough to clip on its own still lands under full scale.
        let mut hot = noise_patch();
        hot.amp = Adsr {
            a: 0.0,
            d: 0.0,
            s: 1.0,
            r: 0.0,
        };
        hot.filter = Some(crate::soundgen::patch::Filter {
            kind: crate::soundgen::patch::FilterKind::Lowpass,
            cutoff: 200.0,
            resonance: 0.99,
            env_amount: 0.0,
            adsr: Adsr::default(),
        });
        let samples = render_limited(&hot, 36.0, &NoteOpts::default()).unwrap();
        let peak = samples.iter().fold(0.0f32, |m, s| m.max(s.abs()));
        assert!(peak <= 1.0, "a bake must never clip (peak {peak})");
    }

    #[test]
    fn an_invalid_patch_or_duration_fails_before_writing_a_file() {
        let path = tmp("rusty_soundgen_never_written.wav");
        std::fs::remove_file(&path).ok();
        let mut broken = noise_patch();
        broken.source = Source::OscStack { oscs: vec![] };
        assert!(bake_note(&broken, 60.0, &NoteOpts::default(), &path).is_err());
        let bad_opts = NoteOpts {
            duration: 0.0,
            ..NoteOpts::default()
        };
        assert!(bake_note(&noise_patch(), 60.0, &bad_opts, &path).is_err());
        assert!(!std::path::Path::new(&path).exists(), "no file on failure");
    }
}
