//! Shared fixtures.

pub(crate) mod songs;
pub(crate) mod spectrum;

use scorsese_zimmer::patch::{Adsr, NoiseColor, Osc, Patch, Source, Wave};
use scorsese_zimmer::{NoteOpts, SynthError};

/// A plain saw through a fully-sustaining envelope — the control case for
/// anything asking what a stage did to the signal.
pub(crate) fn saw_patch() -> Patch {
    Patch {
        layers: Vec::new(),
        source: Source::OscStack {
            oscs: vec![osc(Wave::Saw, 0.0, 0)],
        },
        amp: Adsr {
            a: 0.0,
            d: 0.0,
            s: 1.0,
            r: 0.0,
            curve: 0.0,
        },
        filter: None,
        pitch_env: None,
        lfo: None,
        lfo2: None,
        fx: vec![],
    }
}

/// The smallest legal patch around `source` — every optional stage absent.
pub(crate) fn minimal(source: Source) -> Patch {
    Patch {
        layers: Vec::new(),
        source,
        amp: Adsr::default(),
        filter: None,
        pitch_env: None,
        lfo: None,
        lfo2: None,
        fx: vec![],
    }
}

/// One oscillator, spelled out — a single voice, which is what an `Osc` meant
/// before unison existed.
pub(crate) fn osc(wave: Wave, detune_cents: f32, octave: i32) -> Osc {
    Osc {
        wave,
        detune_cents,
        gain: 1.0,
        octave,
        voices: 1,
        spread: 12.0,
        width: 0.0,
        pulse_width: 0.5,
    }
}

/// The plain hiss: the noise source every test that predates colour meant.
pub(crate) fn noise() -> Source {
    Source::Noise {
        color: NoiseColor::White,
    }
}

/// An ADSR, positionally — the four numbers read better than four fields when
/// a test is about the shape rather than the names.
pub(crate) fn adsr(a: f32, d: f32, s: f32, r: f32) -> Adsr {
    Adsr {
        a,
        d,
        s,
        r,
        curve: 0.0,
    }
}

/// A note of `duration` seconds, struck at full velocity, seed zero, its
/// brightness exactly its level.
pub(crate) fn opts(duration: f32) -> NoteOpts {
    NoteOpts {
        duration,
        velocity: 1.0,
        timbre: 0.0,
        glide: None,
        seed: 0,
    }
}

/// The loudest sample in a buffer, as a magnitude.
pub(crate) fn peak(buf: &[f32]) -> f32 {
    buf.iter()
        .fold(0.0f32, |most, sample| most.max(sample.abs()))
}

/// How many times the signal crosses zero going upward — for a simple waveform
/// this *is* its frequency, which is how a pitch assertion avoids needing an
/// FFT.
pub(crate) fn rising_crossings(buf: &[f32]) -> usize {
    buf.windows(2)
        .filter(|pair| pair[0] <= 0.0 && pair[1] > 0.0)
        .count()
}

/// The pitch of a simple waveform, in Hz, measured *between* its rising
/// crossings.
///
/// Between rather than across the whole buffer because an oscillator starts
/// somewhere in its cycle rather than at zero, so the part-cycle at either end
/// of a window is not a pitch error and must not be counted as one.
pub(crate) fn measured_hz(buf: &[f32], sample_rate: f32) -> f32 {
    let crossings: Vec<usize> = buf
        .windows(2)
        .enumerate()
        .filter(|(_, pair)| pair[0] <= 0.0 && pair[1] > 0.0)
        .map(|(index, _)| index)
        .collect();
    let (first, last) = (
        *crossings.first().expect("the buffer holds a waveform"),
        *crossings.last().expect("the buffer holds a waveform"),
    );
    assert!(last > first, "one crossing says nothing about a period");
    (crossings.len() - 1) as f32 * sample_rate / (last - first) as f32
}

/// High-frequency content: the energy of the first difference relative to the
/// signal's own energy.
///
/// Amplitude-invariant — a quieter signal is not a duller one — which is what
/// makes it a fair brightness measure across a filter.
pub(crate) fn brightness(buf: &[f32]) -> f32 {
    let signal: f32 = buf.iter().map(|sample| sample * sample).sum();
    let edges: f32 = buf.windows(2).map(|w| (w[1] - w[0]).powi(2)).sum();
    if signal > 0.0 { edges / signal } else { 0.0 }
}

/// One channel of an interleaved stereo buffer, `0` being the left.
///
/// Everything this crate renders is stereo, and almost every question a test
/// asks — what pitch is this, how bright is it, when did the envelope open —
/// is a question about a waveform rather than about width. A source produces
/// one waveform and both channels carry it, so one channel is the whole
/// answer, and reading the interleaved buffer as if it were mono would double
/// every measured frequency.
pub(crate) fn channel(buf: &[f32], index: usize) -> Vec<f32> {
    buf.iter().skip(index).step_by(2).copied().collect()
}

/// Renders a note, failing the test with the synth's own words if it will not.
///
/// The **left channel** of it, for the reason [`channel`] gives.
pub(crate) fn render(patch: &Patch, midi: f32, opts: &NoteOpts) -> Vec<f32> {
    channel(&render_stereo(patch, midi, opts), 0)
}

/// Renders a note and hands back both channels, interleaved — for the tests
/// that are about width rather than about waveform.
pub(crate) fn render_stereo(patch: &Patch, midi: f32, opts: &NoteOpts) -> Vec<f32> {
    scorsese_zimmer::render_note(patch, midi, opts).expect("the patch renders")
}

/// The error a patch is refused with — for the cases where *which* refusal
/// matters, not merely that there was one.
pub(crate) fn refusal(patch: &Patch, midi: f32, opts: &NoteOpts) -> SynthError {
    scorsese_zimmer::render_note(patch, midi, opts).expect_err("the patch is refused")
}
