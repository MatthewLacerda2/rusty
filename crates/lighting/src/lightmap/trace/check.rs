//! Which points of an Area light's set a unanimous shadow probe checks
//! (`trace::own`, #2355): a few, stratified over the probe's blocks.

use super::super::lights::area::{block_span, PROBE};
use super::super::rng::Rng;
use super::own::BLOCKS;

/// The checking rays a probe that found a panel fully seen casts among the rest
/// of its set, one in each quadrant of its blocks ([`check_cells`]): 24 rays in
/// all, few because most panels over a floor are fully seen. They are the last
/// chance to find a thin occluder all 20 probe rays missed; one that finds it
/// makes the texel cast the panel's whole set.
pub(super) const LIT_CHECKS: u32 = 4;

/// The checking rays a probe that found a panel fully hidden casts among the
/// rest of its set, two in each block: 52 rays in all, more than [`LIT_CHECKS`]
/// because a hidden panel is rarer over a floor, and a slit the probe missed
/// lights a texel that would otherwise stay black.
pub(super) const HIDDEN_CHECKS: u32 = 2 * BLOCKS as u32;

/// The cells of an `m × m` set a unanimous probe checks, by probe block, none
/// of them the block's probe pick in `picks`: two in every block when hidden,
/// one in a block drawn from each 2 × 2 quadrant of blocks when `seen`.
/// Stratified over the blocks, and inside a block on rows and columns no other
/// of its points took ([`spread`]), so a strip of the panel (a rod's shadow, a
/// slit) is more likely crossed than by as many points drawn at random.
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

/// `take` (1 or 2) cells of probe block `b` of an `m × m` set besides its
/// `pick`, each on a row and a column none taken before it holds where the
/// block is wider than 2 × 2 and has one, else any free one.
fn spread(b: usize, m: u32, pick: u32, take: usize, rng: &mut Rng) -> [u32; 2] {
    let ((x0, x1), (y0, y1)) = (
        block_span(b as u32 % PROBE, m),
        block_span(b as u32 / PROBE, m),
    );
    // In a 2 × 2 block the rule only moves the second check off the diagonal,
    // which measured worse on a slit than any free cell.
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

/// A whole number in `0..n` from `rng`, `n` at least 1.
pub(super) fn below(n: u32, rng: &mut Rng) -> u32 {
    ((rng.next_f32() * n as f32) as u32).min(n - 1)
}
