//! Texels several triangles share in UV0 (#2310): a wall and the panel screwed on in
//! front of it, the two halves of a mirrored model, a level's floor tiling one
//! square. Each such texel takes the claimant a viewer sees, the **more open** one,
//! decided per pair of triangles, not per texel: the walk is run twice. The first
//! traces, on every shared texel, the openness of its owner (the first triangle
//! there in index order) and of each claimant, exactly as the `ao` map traces the
//! texel (its ray count, its stream), and sums them per (owner, claimant) pair.
//! The second gives each shared texel to the claimant whose pair mean beats the
//! owner's by more than the noise ([`tie`]), the widest margin first, else to the
//! owner: so a region two triangles share bakes one of them throughout, where a
//! per-texel pick would speckle between two equally open halves at random. Every
//! map then bakes from that one point, so the maps of a bake agree texel for
//! texel, and the pick is the same whichever maps are asked: a bake without `ao`
//! still traces the shared texels, and only those, and drops the tracer after the
//! pick. A layout with no overlap has no claimant, walks once and traces nothing
//! (its walk still asks, per run of a triangle's texels over another's, whether
//! the two overlap in UV); one whose every pair tied (a mirrored model) keeps the
//! first walk's points and never walks a second time.
//!
//! Memory is bounded by the shared texels and the pairs, never by how many faces
//! stack on one: an `f32` per point (its owner's openness, traced once), the pair
//! sums, and a [`BLOCK`] of claims waiting to be traced. Time is not: a texel `n`
//! faces cover costs `n` traces, so a level mesh tiling UV0 hundreds of times over
//! costs hundreds per texel, said in the bake's log ([`Stacked`]).

use std::collections::BTreeMap;
use std::fmt;

use super::margin::{per_texel, BLOCK};
use super::rays::MeshRays;
use super::Baked;
use crate::lightmap::{rasterize_claims, Claim, TexelPoint};

/// Covering faces per shared texel, on average, from which the bake's log says
/// what the stack costs.
const DEEP: f64 = 8.0;

/// A shared UV area deep enough to tell (more than 8 faces a texel, on
/// average): its texels and the faces covering them, owners included.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stacked {
    pub texels: u64,
    pub faces: u64,
}

impl fmt::Display for Stacked {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mean = self.faces as f64 / self.texels as f64;
        write!(
            f,
            "{} texels of UV0 are each covered by {mean:.1} faces on average (stacked \
             or tiling UVs): each such texel traces `ao_rays` once per face covering it",
            self.texels
        )
    }
}

/// The openness one (owner, claimant) pair of triangles summed over the texels
/// they share.
#[derive(Clone, Copy, Default)]
struct Pair {
    owner: f64,
    claimant: f64,
    texels: u64,
}

/// How much more open a claimant's mean must be than its owner's to take their
/// shared texels: `1 / √ao_rays`, twice the largest standard error of one
/// texel's openness (a share of `ao_rays` rays, at most `0.5 / √ao_rays`). Two
/// mirrored halves, open alike, never part on noise; a panel (1.0) over a wall
/// (≈0) parts at any ray count above one.
fn tie(ao_rays: u32) -> f64 {
    1.0 / f64::from(ao_rays).sqrt()
}

/// The covered texels of `baked`'s model, each shared texel holding its pick's
/// point, and the stack when it is deep. `rays` is built the first time a texel
/// is shared, and kept for the ray-traced maps.
pub(super) fn points(
    baked: &Baked,
    rays: &mut Option<MeshRays>,
) -> (Vec<TexelPoint>, Option<Stacked>) {
    let surface = baked.source.surface();
    let size = baked.settings.size;
    let mut gather = Gather::default();
    let first = rasterize_claims(&surface, size.width, size.height, &mut |c| {
        let rays = rays.get_or_insert_with(|| MeshRays::new(baked.source));
        gather.add(baked, rays, c);
    });
    let Some(rays) = rays.as_ref() else {
        return (first, None);
    };
    gather.flush(baked, rays);
    let texels = gather.open.iter().filter(|v| !v.is_nan()).count() as u64;
    let stacked = Stacked {
        texels,
        faces: texels + gather.claims,
    };
    let deep = (stacked.faces as f64 > DEEP * texels as f64).then_some(stacked);
    let (pairs, tie) = (gather.pairs, tie(baked.settings.ao_rays));
    let mean = |p: &Pair| (p.claimant - p.owner) / p.texels as f64;
    if pairs.values().all(|p| mean(p) <= tie) {
        // Every pair tied (a mirrored model): the owners keep every texel, as the
        // first walk already holds them.
        return (first, deep);
    }
    drop(first);
    let margin = |owner: u32, claimant: u32| pairs.get(&(owner, claimant)).map_or(0.0, mean);
    let points = rasterize_claims(&surface, size.width, size.height, &mut |c: Claim| {
        let held = match c.kept.first == c.owner {
            true => tie,
            false => margin(c.owner, c.kept.first).max(tie),
        };
        if margin(c.owner, c.point.first) > held {
            *c.kept = c.point;
        }
    });
    (points, deep)
}

/// The first walk's sums: per point the owner's openness (NaN until traced), per
/// (owner, claimant) pair the sums, and the claims not yet traced.
#[derive(Default)]
struct Gather {
    open: Vec<f32>,
    pairs: BTreeMap<(u32, u32), Pair>,
    waiting: Vec<(u32, TexelPoint, TexelPoint)>,
    claims: u64,
}

impl Gather {
    /// Hold claim `c`, with its owner's point; trace the waiting claims once a
    /// [`BLOCK`] is held.
    fn add(&mut self, baked: &Baked, rays: &MeshRays, c: Claim) {
        self.waiting.push((c.slot, *c.kept, c.point));
        self.claims += 1;
        if self.waiting.len() == BLOCK {
            self.flush(baked, rays);
        }
    }

    /// Trace the waiting claims and the owners not yet traced, and add them to
    /// their pairs in walk order.
    fn flush(&mut self, baked: &Baked, rays: &MeshRays) {
        let slot = |s: u32| s as usize;
        let mut fresh: Vec<(u32, TexelPoint)> = (self.waiting.iter())
            .filter(|(s, ..)| self.open.get(slot(*s)).is_none_or(|v| v.is_nan()))
            .map(|&(s, owner, _)| (s, owner))
            .collect();
        fresh.sort_unstable_by_key(|&(s, _)| s);
        fresh.dedup_by_key(|&mut (s, _)| s);
        for (&(s, _), v) in fresh.iter().zip(openness(baked, rays, &fresh)) {
            if self.open.len() <= slot(s) {
                self.open.resize(slot(s) + 1, f32::NAN);
            }
            self.open[slot(s)] = v;
        }
        let claimed: Vec<(u32, TexelPoint)> =
            self.waiting.iter().map(|&(s, _, p)| (s, p)).collect();
        for (&(s, owner, p), v) in self.waiting.iter().zip(openness(baked, rays, &claimed)) {
            let pair = self.pairs.entry((owner.first, p.first)).or_default();
            pair.owner += f64::from(self.open[slot(s)]);
            pair.claimant += f64::from(v);
            pair.texels += 1;
        }
        self.waiting.clear();
    }
}

/// The `ao` openness of each (place in the points, point), traced as the `ao` map
/// traces that place.
fn openness(baked: &Baked, rays: &MeshRays, at: &[(u32, TexelPoint)]) -> Vec<f32> {
    let points: Vec<TexelPoint> = at.iter().map(|&(_, p)| p).collect();
    let traced = per_texel(&points, |k, p| rays.ao(baked, at[k].0 as usize, p));
    traced.iter().map(|v| v.x).collect()
}
