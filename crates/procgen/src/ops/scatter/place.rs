//! Where `scatter`'s stamps sit (#2187): hashed points on the torus, each stamp's
//! from its own index, kept apart by `spacing` with a deterministic dart throw.

use super::super::super::hash;

/// The hash channels a stamp's candidate points draw from (clear of `tile`'s
/// 20–26); the stamp's other draws take [`super::DRAWS`] onward.
const POS_X: u64 = 40;
const POS_Y: u64 = 41;

/// The candidates a stamp tries before it gives up its place under `spacing`.
const CANDIDATES: i64 = 32;

/// The finest grid the dart throw files earlier stamps in: past it a cell holds
/// several stamps, which only costs time.
const MAX_GRID: usize = 256;

/// The closest two stamp centres may sit, in tiles: `spacing` times the smaller of
/// the stamp's size and `√(0.5 / amount)`, the gap at which discs of that
/// diameter cover about 40% of the tile, where the dart throw still finds room for
/// nearly every stamp. So pebbles stop touching, and a crowd too dense to keep its
/// stamps' size apart still spreads evenly rather than losing most of its stamps.
pub(super) fn gap(spacing: f32, amount: u32, size: f32) -> f32 {
    if spacing <= 0.0 || amount == 0 {
        return 0.0;
    }
    let crowd = (0.5 / amount as f32).sqrt();
    spacing * size.clamp(0.0, 1.0).min(crowd)
}

/// Stamp `i`'s candidate `j`: a point uniform over the tile.
fn candidate(i: u32, j: i64, seed: u64) -> [f32; 2] {
    let unit = |ch| hash::unit2(i64::from(i), j, ch, seed);
    [unit(POS_X), unit(POS_Y)]
}

/// The distance squared between two points of the torus.
fn dist2(a: [f32; 2], b: [f32; 2]) -> f32 {
    let d = |a: f32, b: f32| {
        let d = (a - b).abs();
        d.min(1.0 - d)
    };
    d(a[0], b[0]).powi(2) + d(a[1], b[1]).powi(2)
}

/// The centres of `amount` stamps, by index. With `gap = 0` stamp `i` sits at its
/// first candidate; otherwise at the first of its candidates at least `gap` from
/// every earlier placed stamp, or nowhere (`None`) when all of them are closer.
/// A stamp reads only its own hashes and the stamps before it, so raising
/// `amount` adds stamps without moving the others.
pub(super) fn place(amount: u32, gap: f32, seed: u64) -> Vec<Option<[f32; 2]>> {
    if gap <= 0.0 {
        return (0..amount).map(|i| Some(candidate(i, 0, seed))).collect();
    }
    let g = ((1.0 / gap) as usize).clamp(1, MAX_GRID);
    let mut cells: Vec<Vec<[f32; 2]>> = vec![Vec::new(); g * g];
    let cell = |p: [f32; 2]| p.map(|c| ((c * g as f32) as usize).min(g - 1));
    let near = |cells: &Vec<Vec<[f32; 2]>>, p: [f32; 2]| {
        let [x, y] = cell(p);
        // A cell is at least `gap` wide, so only the 3 × 3 around `p` can be too
        // close; under 3 cells across those wrap onto each other, so look at all.
        let reach: Vec<usize> = match g < 3 {
            true => (0..g * g).collect(),
            false => (0..9)
                .map(|k| ((y + g + k / 3 - 1) % g) * g + (x + g + k % 3 - 1) % g)
                .collect(),
        };
        reach
            .into_iter()
            .any(|c| cells[c].iter().any(|&q| dist2(p, q) < gap * gap))
    };
    (0..amount)
        .map(|i| {
            let p = (0..CANDIDATES)
                .map(|j| candidate(i, j, seed))
                .find(|&p| !near(&cells, p))?;
            let [x, y] = cell(p);
            cells[y * g + x].push(p);
            Some(p)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_gap_is_the_smaller_of_the_size_and_the_packing() {
        assert_eq!(gap(0.0, 100, 0.1), 0.0);
        assert_eq!(gap(1.0, 0, 0.1), 0.0);
        assert_eq!(gap(1.0, 4, 0.1), 0.1, "4 stamps spread far wider than 0.1");
        assert_eq!(
            gap(0.5, 200, 0.2),
            0.5 * 0.05,
            "200 stamps crowd to √(1/400)"
        );
    }

    #[test]
    fn the_torus_distance_wraps() {
        assert!((dist2([0.05, 0.5], [0.95, 0.5]) - 0.01).abs() < 1e-6);
    }
}
