use std::collections::BTreeMap;
use std::fmt;

use super::margin::{per_texel, BLOCK};
use super::rays::{MeshRays, MAX_DISTANCE};
use super::Baked;
use crate::lightmap::{rasterize_claims, Claim, TexelPoint};

/// Covering faces per shared texel, on average, past which the log reports the stack's cost.
const DEEP: f64 = 8.0;

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

#[derive(Clone, Copy, Default)]
struct Pair {
    owner: f64,
    claimant: f64,
    texels: u64,
}

fn tie(ao_rays: u32) -> f64 {
    1.0 / f64::from(ao_rays).sqrt()
}

pub(super) fn points(
    baked: &Baked,
    rays: &mut Option<MeshRays>,
) -> (Vec<TexelPoint>, Option<Stacked>) {
    let surface = baked.written();
    let size = baked.settings.size;
    let mut gather = Gather::default();
    let first = rasterize_claims(&surface, size.width, size.height, &mut |c| {
        let rays = rays.get_or_insert_with(|| MeshRays::new(baked.source, &baked.settings.faces));
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

#[derive(Default)]
struct Gather {
    open: Vec<f32>,
    pairs: BTreeMap<(u32, u32), Pair>,
    waiting: Vec<(u32, TexelPoint, TexelPoint)>,
    claims: u64,
}

impl Gather {
    fn add(&mut self, baked: &Baked, rays: &MeshRays, c: Claim) {
        self.waiting.push((c.slot, *c.kept, c.point));
        self.claims += 1;
        if self.waiting.len() == BLOCK {
            self.flush(baked, rays);
        }
    }

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

/// Each face's openness at a shared texel, traced to the whole diagonal whatever `ao_distance`
/// says: the setting bounds the `ao` map, never which face every map bakes.
fn openness(baked: &Baked, rays: &MeshRays, at: &[(u32, TexelPoint)]) -> Vec<f32> {
    let points: Vec<TexelPoint> = at.iter().map(|&(_, p)| p).collect();
    let reach = MAX_DISTANCE * baked.diagonal;
    let traced = per_texel(&points, |k, p| rays.open(baked, at[k].0 as usize, p, reach));
    traced.iter().map(|v| v.x).collect()
}
