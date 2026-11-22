use super::super::lights::area::{block_span, PROBE};
use super::super::rng::Rng;
use super::own::BLOCKS;

/// Checks a probe that found a panel fully seen adds, one per quadrant: few, as most are.
pub(super) const LIT_CHECKS: u32 = 4;

/// Two per block, more than `LIT_CHECKS`: a slit the probe missed would leave a texel black.
pub(super) const HIDDEN_CHECKS: u32 = 2 * BLOCKS as u32;

pub(super) fn check_cells(
    m: u32,
    picks: &[u32; BLOCKS],
    seen: bool,
    rng: &mut Rng,
) -> [[u32; 2]; BLOCKS] {
    let mut cells = [[u32::MAX; 2]; BLOCKS];
    if seen {
        for quadrant in 0..4 {
            let bx = quadrant % 2 * 2 + below(2, rng);
            let by = quadrant / 2 * 2 + below(2, rng);
            let b = (by * PROBE + bx) as usize;
            cells[b] = spread(b, m, picks[b], 1, rng);
        }
    } else {
        for (b, cell) in cells.iter_mut().enumerate() {
            *cell = spread(b, m, picks[b], 2, rng);
        }
    }
    cells
}

fn spread(b: usize, m: u32, pick: u32, take: usize, rng: &mut Rng) -> [u32; 2] {
    let ((x0, x1), (y0, y1)) = (
        block_span(b as u32 % PROBE, m),
        block_span(b as u32 / PROBE, m),
    );
    // Not on a 2×2 block: there the rule only moves the second check off the diagonal, which
    // measured worse on a slit.
    let wide = x1 - x0 > 2 && y1 - y0 > 2;
    let block = move || (y0..y1).flat_map(move |y| (x0..x1).map(move |x| y * m + x));
    let mut taken = [pick, u32::MAX, u32::MAX];
    for t in 1..=take {
        let held = &taken[..t];
        let apart = |c: u32| held.iter().all(|&h| h % m != c % m && h / m != c / m);
        let strict = wide && block().any(apart);
        let ok = |c: &u32| !held.contains(c) && (!strict || apart(*c));
        let n = block().filter(ok).count() as u32;
        taken[t] = block()
            .filter(ok)
            .nth(below(n, rng) as usize)
            .unwrap_or(u32::MAX);
    }
    [taken[1], taken[2]]
}

pub(super) fn below(n: u32, rng: &mut Rng) -> u32 {
    ((rng.next_f32() * n as f32) as u32).min(n - 1)
}
