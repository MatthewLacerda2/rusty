//! Scoring and picking (#1892): normalize, shape, weigh, sum, rank; then choose.

use glam::Vec3;

use super::spec::{QuerySpec, RunMode};
use super::Item;
use crate::core::random::Random;

/// The surviving items `alive` ranked best first, a tie to the earliest
/// generated. A scoring test normalizes its raw values to 0..1 over the
/// survivors; when they are all equal it skips the curve and gives each its
/// full weight (no division by zero, and a test that cannot tell items apart
/// favours none, whatever its curve).
pub fn rank(spec: &QuerySpec, items: &[Vec3], alive: &[usize], raws: &[Vec<f32>]) -> Vec<Item> {
    let mut scores = vec![0.0f32; alive.len()];
    for (test, row) in spec.tests.iter().zip(raws) {
        let Some(score) = test.score else { continue };
        let values: Vec<f32> = alive.iter().map(|&i| row[i]).collect();
        let lo = values.iter().copied().fold(f32::INFINITY, f32::min);
        let hi = values.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        let spread = hi > lo && (hi - lo).is_finite();
        for (s, v) in scores.iter_mut().zip(&values) {
            // Every survivor alike: no curve, each takes the full weight.
            let shaped = if spread {
                score.curve.shape(((v - lo) / (hi - lo)).clamp(0.0, 1.0))
            } else {
                1.0
            };
            *s += score.weight * shaped;
        }
    }
    let mut ranked: Vec<(usize, f32)> = alive.iter().copied().zip(scores).collect();
    ranked.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    ranked
        .into_iter()
        .map(|(i, score)| Item {
            position: items[i],
            score,
        })
        .collect()
}

/// The item `mode` picks from `ranked` (best first): the best, or a random one
/// among those scoring in the top 5% / 25% of the score range (Unreal's
/// `RandomBest5Pct` / `RandomBest25Pct`). A random mode draws exactly once from
/// `rng` when there is anything to pick, and never when `ranked` is empty.
pub fn pick(ranked: &[Item], mode: RunMode, rng: &mut Random) -> Option<Item> {
    let best = *ranked.first()?;
    let share = match mode {
        RunMode::Best => return Some(best),
        RunMode::RandomBest5 => 0.05,
        RunMode::RandomBest25 => 0.25,
    };
    let worst = ranked.last().map_or(best.score, |i| i.score);
    let floor = best.score - share * (best.score - worst);
    let top = ranked
        .iter()
        .take_while(|i| i.score >= floor)
        .count()
        .max(1);
    let k = rng.range_i64(0, top as i64).clamp(0, top as i64 - 1);
    ranked.get(k as usize).copied()
}
