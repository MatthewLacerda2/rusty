//! Another take of one patch (#1038): through the whole signal path, two
//! seeds of a varied one-shot differ in **envelope**, not merely in grain —
//! and without `vary` they are still the same sound.

use crate::common::{adsr, minimal, osc, render};
use scorsese_zimmer::patch::{PitchEnv, Source, Wave};
use scorsese_zimmer::{NoteOpts, Patch};

/// A pop: a sine falling an octave onto its note, gone in a tenth of a second.
fn pop() -> Patch {
    let mut amp = adsr(0.001, 0.07, 0.0, 0.02);
    amp.curve = 4.0;
    let mut bounce = adsr(0.0, 0.03, 0.0, 0.0);
    bounce.curve = 3.0;
    Patch {
        amp,
        pitch_env: Some(PitchEnv {
            semitones: -12.0,
            adsr: bounce,
        }),
        ..minimal(Source::OscStack {
            oscs: vec![osc(Wave::Sine, 0.0, 0)],
        })
    }
}

/// The level, 5 ms at a time — what the ear hears as the shape of a hit.
fn envelope(patch: &Patch, seed: u64) -> Vec<f32> {
    let opts = NoteOpts {
        duration: 0.1,
        seed,
        ..NoteOpts::default()
    };
    let mut buf = render(patch, 72.0, &opts);
    buf.resize(44_100 / 4, 0.0);
    buf.chunks(220)
        .map(|w| (w.iter().map(|s| s * s).sum::<f32>() / w.len() as f32).sqrt())
        .collect()
}

/// Pearson correlation of two envelopes.
fn correlation(a: &[f32], b: &[f32]) -> f32 {
    let mean = |x: &[f32]| x.iter().sum::<f32>() / x.len() as f32;
    let (ma, mb) = (mean(a), mean(b));
    let dot = |x: &[f32], mx: f32, y: &[f32], my: f32| {
        x.iter()
            .zip(y)
            .map(|(p, q)| (p - mx) * (q - my))
            .sum::<f32>()
    };
    dot(a, ma, b, mb) / (dot(a, ma, a, ma) * dot(b, mb, b, mb)).sqrt()
}

/// The lowest correlation between seed 0's envelope and seeds 1 to 7's.
fn least_alike(vary: f32) -> f32 {
    let first = envelope(&pop().varied(vary, 0), 0);
    (1..8)
        .map(|seed| correlation(&first, &envelope(&pop().varied(vary, seed), seed)))
        .fold(1.0, f32::min)
}

#[test]
fn a_varied_one_shot_changes_shape_from_seed_to_seed() {
    let same = least_alike(0.0);
    let varied = least_alike(1.0);
    assert!(
        same > 0.99,
        "without vary the takes already differ ({same})"
    );
    assert!(
        varied < 0.95,
        "seven varied takes all kept one shape ({varied})"
    );
}
