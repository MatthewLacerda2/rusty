//! Dropouts and stepped gain, both read off a channel's energy every half millisecond.

use super::quadrature::Quadrature;
use super::Found;

const BLOCK: f64 = 0.0005;

const DEPTH_DB: f64 = 40.0;
const SOUNDING: f64 = 1e-4;
const AROUND: f64 = 0.02;
const SHORTEST_DROPOUT: f64 = 0.001;
const LONGEST_DROPOUT: f64 = 0.04;

#[derive(Debug, Clone)]
pub(super) struct Envelope {
    block: usize,
    quadrature: Quadrature,
    energy: Vec<f64>,
    sum: f64,
    envelope: Vec<f64>,
    envelope_sum: f64,
    filled: usize,
}

impl Envelope {
    pub(super) fn new(rate: u32) -> Self {
        Self {
            block: ((BLOCK * f64::from(rate)).round() as usize).max(1),
            quadrature: Quadrature::default(),
            energy: Vec::new(),
            sum: 0.0,
            envelope: Vec::new(),
            envelope_sum: 0.0,
            filled: 0,
        }
    }

    pub(super) fn feed(&mut self, sample: f32) {
        self.sum += f64::from(sample) * f64::from(sample);
        self.envelope_sum += self.quadrature.energy(sample);
        self.filled += 1;
        if self.filled == self.block {
            self.energy.push(self.sum / self.block as f64);
            self.envelope.push(self.envelope_sum / self.block as f64);
            self.sum = 0.0;
            self.envelope_sum = 0.0;
            self.filled = 0;
        }
    }

    fn frame(&self, block: usize) -> u64 {
        (block * self.block) as u64
    }

    fn blocks(&self, seconds: f64) -> usize {
        ((seconds / BLOCK).round() as usize).max(1)
    }

    fn mean(&self, blocks: std::ops::Range<usize>) -> f64 {
        let len = blocks.len().max(1) as f64;
        self.energy[blocks].iter().sum::<f64>() / len
    }

    pub(super) fn dropouts(&self) -> Vec<Found> {
        let (around, shortest, longest) = (
            self.blocks(AROUND),
            self.blocks(SHORTEST_DROPOUT),
            self.blocks(LONGEST_DROPOUT),
        );
        let deep = 10f64.powf(-DEPTH_DB / 10.0);
        let mut found = Vec::new();
        let mut at = around;
        while at + around < self.energy.len() {
            let before = self.mean(at - around..at);
            if before < SOUNDING || self.energy[at] >= before * deep {
                at += 1;
                continue;
            }
            let end = (at..self.energy.len())
                .find(|&k| self.energy[k] >= before * deep)
                .unwrap_or(self.energy.len());
            let length = end - at;
            if (shortest..=longest).contains(&length) && end + around <= self.energy.len() {
                let after = self.mean(end..end + around);
                let gap = self.mean(at..end).max(f64::MIN_POSITIVE);
                let size_db = 10.0 * (before.min(after) / gap).log10();
                if after >= SOUNDING && size_db >= DEPTH_DB {
                    found.push(Found {
                        at: self.frame(at),
                        from: self.frame(at),
                        to: self.frame(end),
                        size_db: size_db.min(120.0),
                        period: None,
                    });
                }
            }
            at = end.max(at + 1);
        }
        found
    }

    pub(super) fn zipper(&self) -> Vec<Found> {
        super::zipper::staircases(&self.envelope, self.block)
    }
}

#[cfg(test)]
mod tests {
    use super::{Envelope, SOUNDING};

    #[test]
    fn a_gap_after_sound_at_exactly_the_floor_is_a_dropout() {
        let mut envelope = Envelope::new(48_000);
        let around = envelope.blocks(super::AROUND);
        let most = vec![SOUNDING; around - 1];
        let last = SOUNDING * around as f64 - most.iter().sum::<f64>();
        let sound = [most, vec![last]].concat();
        envelope.energy.clone_from(&sound);
        assert_eq!(envelope.mean(0..around), SOUNDING);
        envelope.energy = [sound, vec![0.0; 10], vec![1e-2; around]].concat();
        let found = envelope.dropouts();
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].at, (around * envelope.block) as u64);
    }
}
