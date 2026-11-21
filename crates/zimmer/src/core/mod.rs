//! core — the note renderer: patch + note → a stereo pair of `f32` buffers.

pub(crate) mod additive;
pub(crate) mod env;
pub(crate) mod filter;
pub(crate) mod fm;
pub(crate) mod karplus;
pub(crate) mod lfo;
pub(crate) mod noise;
pub(crate) mod nyquist;
pub(crate) mod osc;
pub(crate) mod source;
pub(crate) mod sweep;
pub(crate) mod tracks;
pub(crate) mod trim;
mod voice;

use super::error::SynthError;
use super::fx;
use super::note::NoteOpts;
use super::patch::Patch;
use super::stereo::Stereo;
use lfo::{Lfos, Onset};
use voice::{layer_seed, Struck, Voice};

pub const SAMPLE_RATE: u32 = 44_100;

/// What a sound source is told of the note it renders: when it is let go, its seed and the
/// sample rate.
#[derive(Clone, Copy, Debug)]
pub(crate) struct NoteContext {
    pub(crate) gate: f32,
    pub(crate) seed: u64,
    pub(crate) rate: f32,
}

pub(crate) const RATE: f32 = SAMPLE_RATE as f32;

const MAX_SECONDS: f32 = 60.0;

pub(crate) fn render_note(patch: &Patch, midi: f32, opts: &NoteOpts) -> Result<Stereo, SynthError> {
    render_swept(patch, midi, opts, None, None)
}

pub(crate) fn render_swept(
    patch: &Patch,
    midi: f32,
    opts: &NoteOpts,
    sweep: Option<&dyn Fn(usize) -> f32>,
    onset: Option<&Onset<'_>>,
) -> Result<Stereo, SynthError> {
    match onset {
        Some(_) => patch.validate_in_song()?,
        None => patch.validate()?,
    }
    let gate = gate_length(opts.duration)?;
    let release = patch
        .layers
        .iter()
        .fold(patch.amp.r.max(0.0), |most, layer| most.max(layer.amp.r));
    let n = sample_count(gate, release + fx::tail_seconds(&patch.fx, &fx::NoTempo))?;
    let lfos = Lfos::of(patch.lfos(), opts.seed, onset, n);
    let note = Struck {
        midi,
        opts,
        pulse: osc::pulse::track(&lfos, n),
        lfos,
        gate,
        n,
    };
    let base = sweep.map(|sweep| sweep::swept(sweep, n));
    let mut buf = note.voice(&Voice::top(patch), opts.seed, base.as_deref());
    for (index, layer) in patch.layers.iter().enumerate() {
        let part = note.voice(&Voice::layer(layer), layer_seed(opts.seed, index), None);
        buf.add(&part);
    }
    fx::apply_chain(&mut buf, &patch.fx, RATE);
    fx::close_tail(&mut buf, &patch.fx, RATE, &fx::NoTempo);
    Ok(buf)
}

fn gate_length(duration: f32) -> Result<f32, SynthError> {
    if !duration.is_finite() || duration <= 0.0 {
        return Err(SynthError::BadDuration { duration });
    }
    Ok(duration)
}

pub(crate) fn render_limited(
    patch: &Patch,
    midi: f32,
    opts: &NoteOpts,
) -> Result<Stereo, SynthError> {
    let mut buf = render_note(patch, midi, opts)?;
    fx::limiter::apply(&mut buf, RATE);
    trim::to_floor(&mut buf);
    Ok(buf)
}

fn sample_count(held: f32, tail: f32) -> Result<usize, SynthError> {
    let seconds = held + tail;
    if !(seconds.is_finite() && seconds <= MAX_SECONDS) {
        return Err(SynthError::NoteTooLong {
            at: None,
            held,
            tail,
            limit: MAX_SECONDS,
        });
    }
    Ok(((seconds * RATE).ceil() as usize).max(1))
}
