//! Shared fixtures.

pub(crate) mod songs;
pub(crate) mod spectrum;

use scorsese_zimmer::patch::{Adsr, NoiseColor, Osc, Patch, Source, Wave};
use scorsese_zimmer::{NoteOpts, SynthError};

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

pub(crate) fn noise() -> Source {
    Source::Noise {
        color: NoiseColor::White,
    }
}

pub(crate) fn adsr(a: f32, d: f32, s: f32, r: f32) -> Adsr {
    Adsr {
        a,
        d,
        s,
        r,
        curve: 0.0,
    }
}

pub(crate) fn opts(duration: f32) -> NoteOpts {
    NoteOpts {
        duration,
        velocity: 1.0,
        timbre: 0.0,
        glide: None,
        seed: 0,
    }
}

pub(crate) fn peak(buf: &[f32]) -> f32 {
    buf.iter()
        .fold(0.0f32, |most, sample| most.max(sample.abs()))
}

pub(crate) fn rising_crossings(buf: &[f32]) -> usize {
    buf.windows(2)
        .filter(|pair| pair[0] <= 0.0 && pair[1] > 0.0)
        .count()
}

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

pub(crate) fn brightness(buf: &[f32]) -> f32 {
    let signal: f32 = buf.iter().map(|sample| sample * sample).sum();
    let edges: f32 = buf.windows(2).map(|w| (w[1] - w[0]).powi(2)).sum();
    if signal > 0.0 { edges / signal } else { 0.0 }
}

pub(crate) fn channel(buf: &[f32], index: usize) -> Vec<f32> {
    buf.iter().skip(index).step_by(2).copied().collect()
}

pub(crate) fn render(patch: &Patch, midi: f32, opts: &NoteOpts) -> Vec<f32> {
    channel(&render_stereo(patch, midi, opts), 0)
}

pub(crate) fn render_stereo(patch: &Patch, midi: f32, opts: &NoteOpts) -> Vec<f32> {
    scorsese_zimmer::render_note(patch, midi, opts).expect("the patch renders")
}

pub(crate) fn refusal(patch: &Patch, midi: f32, opts: &NoteOpts) -> SynthError {
    scorsese_zimmer::render_note(patch, midi, opts).expect_err("the patch is refused")
}
