//! lfo: a patch's LFOs, worked out over one note.
//!
//! An [`Lfo`] is a description; what the stages walk is its value at each
//! sample of the note, and that is decided once here, before any layer is
//! rendered, so every layer of the note moves together. Each LFO becomes two
//! curves: the **wave** in `−1..=1`, and the **depth** at each sample, which
//! is nothing through `delay`, grows over `fade` and is the written depth
//! after. The readers in [`super::tracks`] and the pulse width combine them
//! the way each target says: pitch, cutoff and width offsets add, the amp's
//! dips multiply.
//!
//! **Where a cycle starts.** A `rate` LFO starts at zero when its delay ends,
//! on every note — which, with no delay, is the note's first sample, as it
//! always was. A `beats` LFO reads its phase off the song's clock at the
//! frame the note sounds, so it lands on the same point of the beat whichever
//! note retriggers it, and follows a tempo change as it goes.

use std::f32::consts::TAU;

use super::RATE;
use crate::hash::unit2;
use crate::patch::{Lfo, LfoShape, LfoTarget};

/// Hash channel a `random` LFO draws its held values on, apart from every
/// other draw the note's seed feeds.
const RANDOM_CHANNEL: u64 = 0x4c46; // "LF"

/// What turns a frame of a song into a beat: the song's clock. A trait for the
/// reason [`fx::Tempo`](crate::fx::Tempo) is one: the note renderer has never
/// heard of a song.
pub(crate) trait Grid {
    /// Which beat of the piece sample-frame `frame` falls on.
    fn beat_at(&self, frame: usize) -> f32;
}

/// Where a note sounds in a song: the clock it is played against and the
/// frame of the piece its first sample lands on.
pub(crate) struct Onset<'a> {
    pub(crate) grid: &'a dyn Grid,
    pub(crate) frame: usize,
}

/// One LFO over one note.
struct Moving {
    target: LfoTarget,
    depth: Vec<f32>,
    wave: Vec<f32>,
}

/// Every LFO of a patch over one note, in the order the patch writes them.
pub(crate) struct Lfos(Vec<Moving>);

impl Lfos {
    /// `lfos` worked out over a note `n` frames long struck with `seed`, and
    /// sounding at `onset` in a song — or on its own, where a `beats` LFO,
    /// which validation refuses there, holds still.
    pub(crate) fn of<'a>(
        lfos: impl IntoIterator<Item = &'a Lfo>,
        seed: u64,
        onset: Option<&Onset<'_>>,
        n: usize,
    ) -> Self {
        let moving = lfos.into_iter().enumerate();
        Self(
            moving
                .map(|(index, lfo)| Moving {
                    target: lfo.target,
                    depth: depth(lfo, n),
                    wave: wave(lfo, index, seed, onset, n),
                })
                .collect(),
        )
    }

    /// Whether any LFO is aimed at `target`.
    pub(crate) fn aims_at(&self, target: LfoTarget) -> bool {
        self.0.iter().any(|lfo| lfo.target == target)
    }

    /// The summed swing of every LFO aimed at `target` at sample `i`: each its
    /// depth times its wave, in the target's own unit.
    pub(crate) fn offset(&self, target: LfoTarget, i: usize) -> f32 {
        self.0
            .iter()
            .filter(|lfo| lfo.target == target)
            .map(|lfo| lfo.depth[i] * lfo.wave[i])
            .sum()
    }

    /// The tremolo gain at sample `i`: each LFO aimed at `amp` dips the level
    /// by its depth (so `depth = 1` dips all the way to silence), and two dips
    /// multiply.
    pub(crate) fn tremolo(&self, i: usize) -> f32 {
        self.0
            .iter()
            .filter(|lfo| lfo.target == LfoTarget::Amp)
            .map(|lfo| 1.0 - lfo.depth[i].clamp(0.0, 1.0) * 0.5 * (1.0 - lfo.wave[i]))
            .product()
    }
}

/// The frame `lfo`'s delay ends on.
fn delay_frames(lfo: &Lfo) -> usize {
    (lfo.delay.max(0.0) * RATE).round() as usize
}

/// `lfo`'s depth at each of `n` samples: nothing through the delay, a straight
/// line up over the fade, the written depth after.
fn depth(lfo: &Lfo, n: usize) -> Vec<f32> {
    let start = delay_frames(lfo);
    let fade = lfo.fade.max(0.0) * RATE;
    (0..n)
        .map(|i| match i.checked_sub(start) {
            None => 0.0,
            Some(k) if (k as f32) < fade => lfo.depth * (k as f32 / fade),
            Some(_) => lfo.depth,
        })
        .collect()
}

/// `lfo`'s wave at each of `n` samples.
fn wave(lfo: &Lfo, index: usize, seed: u64, onset: Option<&Onset<'_>>, n: usize) -> Vec<f32> {
    let start = delay_frames(lfo);
    (0..n)
        .map(|i| match (lfo.beats, onset) {
            (Some(beats), Some(onset)) => {
                let cycles = onset.grid.beat_at(onset.frame + i) / beats;
                shape(lfo.shape, cycles, index, seed)
            }
            (Some(_), None) => 0.0,
            (None, _) => match i.checked_sub(start) {
                None => 0.0,
                // The angle in the order the one sine always worked it out,
                // so a sine LFO with no delay renders the samples it did.
                Some(k) => match lfo.shape {
                    LfoShape::Sine => (TAU * lfo.rate.max(0.0) * k as f32 / RATE).sin(),
                    other => shape(other, lfo.rate.max(0.0) * k as f32 / RATE, index, seed),
                },
            },
        })
        .collect()
}

/// The value of `shape` `cycles` cycles in, in `−1..=1`. `index` is which of
/// the patch's LFOs it is, so two `random` ones draw apart.
fn shape(shape: LfoShape, cycles: f32, index: usize, seed: u64) -> f32 {
    let p = cycles.rem_euclid(1.0);
    match shape {
        LfoShape::Sine => (TAU * p).sin(),
        LfoShape::Triangle if p < 0.25 => 4.0 * p,
        LfoShape::Triangle if p < 0.75 => 2.0 - 4.0 * p,
        LfoShape::Triangle => 4.0 * p - 4.0,
        LfoShape::SawUp => 2.0 * (p + 0.5).fract() - 1.0,
        LfoShape::SawDown => 1.0 - 2.0 * (p + 0.5).fract(),
        LfoShape::Square if p < 0.5 => 1.0,
        LfoShape::Square => -1.0,
        LfoShape::Random => {
            let cycle = cycles.floor() as i64;
            2.0 * unit2(cycle, index as i64, RANDOM_CHANNEL, seed) - 1.0
        }
    }
}

#[cfg(test)]
mod tests;
