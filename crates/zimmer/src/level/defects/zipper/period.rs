//! The level steps of a signal and the periods they repeat at.

use super::{LONGEST, REPEATS, SHORTEST, SILENT, STEPS, TRIED, WINDOW};

pub(super) fn candidates(steps: &[f64]) -> Vec<usize> {
    let longest = LONGEST.min(steps.len() / STEPS);
    if longest <= SHORTEST {
        return Vec::new();
    }
    let ac: Vec<f64> = (0..=longest + 1)
        .map(|lag| {
            steps
                .iter()
                .zip(&steps[lag.min(steps.len())..])
                .map(|(a, b)| a * b)
                .sum()
        })
        .collect();
    let mut peaks: Vec<usize> = (SHORTEST..=longest)
        .filter(|&lag| {
            ac[lag] >= REPEATS * ac[0] && ac[lag] >= ac[lag - 1] && ac[lag] >= ac[lag + 1]
        })
        .collect();
    peaks.sort_by(|&a, &b| ac[b].total_cmp(&ac[a]));
    peaks.truncate(TRIED);
    peaks
}

pub(super) fn steps(energy: &[f64]) -> Vec<f64> {
    let weights: Vec<f64> = (0..WINDOW)
        .map(|j| 0.5 - 0.5 * (std::f64::consts::TAU * (j as f64 + 0.5) / WINDOW as f64).cos())
        .collect();
    let side = |from: usize| -> f64 {
        weights
            .iter()
            .zip(&energy[from..from + WINDOW])
            .map(|(w, e)| w * e)
            .sum()
    };
    (0..energy.len())
        .map(|at| {
            if at < WINDOW || at + WINDOW > energy.len() {
                return 0.0;
            }
            let (before, after) = (side(at - WINDOW), side(at));
            if before < SILENT || after < SILENT {
                return 0.0;
            }
            10.0 * (after / before).log10()
        })
        .collect()
}
