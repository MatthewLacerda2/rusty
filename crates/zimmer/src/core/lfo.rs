//! lfo: a patch's LFOs, worked out over one note.

use std::f32::consts::TAU;

use super::RATE;
use crate::hash::unit2;
use crate::patch::{Lfo, LfoShape, LfoTarget};

/// Hash channel ("LF") a `random` LFO draws on, apart from every other draw of the seed.
const RANDOM_CHANNEL: u64 = 0x4c46;

pub(crate) trait Grid {
    fn beat_at(&self, frame: usize) -> f32;
}

pub(crate) struct Onset<'a> {
    pub(crate) grid: &'a dyn Grid,
    pub(crate) frame: usize,
}

struct Moving {
    target: LfoTarget,
    depth: Vec<f32>,
    wave: Vec<f32>,
}

pub(crate) struct Lfos(Vec<Moving>);

impl Lfos {
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

    pub(crate) fn aims_at(&self, target: LfoTarget) -> bool {
        self.0.iter().any(|lfo| lfo.target == target)
    }

    pub(crate) fn offset(&self, target: LfoTarget, i: usize) -> f32 {
        self.0
            .iter()
            .filter(|lfo| lfo.target == target)
            .map(|lfo| lfo.depth[i] * lfo.wave[i])
            .sum()
    }

    pub(crate) fn tremolo(&self, i: usize) -> f32 {
        self.0
            .iter()
            .filter(|lfo| lfo.target == LfoTarget::Amp)
            .map(|lfo| 1.0 - lfo.depth[i].clamp(0.0, 1.0) * 0.5 * (1.0 - lfo.wave[i]))
            .product()
    }
}

fn delay_frames(lfo: &Lfo) -> usize {
    (lfo.delay.max(0.0) * RATE).round() as usize
}

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

fn wave(lfo: &Lfo, index: usize, seed: u64, onset: Option<&Onset<'_>>, n: usize) -> Vec<f32> {
    let start = delay_frames(lfo);
    (0..n)
        .map(|i| match (lfo.beats, onset) {
            (Some(beats), Some(onset)) => {
                let cycles = onset.grid.beat_at(onset.frame + i) / beats;
                shape(lfo.shape, cycles, index, seed)
            }
            // A note outside a song: `Patch::validate` refuses `beats` there, so this holds still.
            (Some(_), None) => 0.0,
            (None, _) => match i.checked_sub(start) {
                None => 0.0,
                // The angle in the order the one sine always worked it out, so a sine LFO with no
                // delay renders the samples it did.
                Some(k) => match lfo.shape {
                    LfoShape::Sine => (TAU * lfo.rate.max(0.0) * k as f32 / RATE).sin(),
                    other => shape(other, lfo.rate.max(0.0) * k as f32 / RATE, index, seed),
                },
            },
        })
        .collect()
}

/// In `−1..=1`; `index` keeps two `random` LFOs drawing apart.
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
