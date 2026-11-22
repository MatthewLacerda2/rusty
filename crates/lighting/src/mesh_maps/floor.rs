use std::fmt;

use super::Baked;
use crate::lightmap::Areas;
use rusty_core::collections::Map;

/// The narrowest band, in the island's texels.
pub(super) const MIN_TEXELS: f32 = 3.0;

pub(super) struct Radii {
    pub per_tri: Vec<f32>,
    pub reach: f32,
    pub raised: Option<Raised>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Raised {
    pub requested: f32,
    pub low: f32,
    pub high: f32,
    pub islands: usize,
    pub of: usize,
}

impl Radii {
    pub(super) fn new(baked: &Baked) -> Self {
        let source = baked.source;
        let surface = source.surface();
        let islands = source.islands();
        let mut areas: Map<u32, Areas> = Map::default();
        for (t, &island) in source.indices.chunks_exact(3).zip(&islands) {
            areas.entry(island).or_default().add(&surface, t);
        }
        let reach = baked.settings.curvature_reach(baked.diagonal);
        let average = baked.texel_world();
        let floors: Map<u32, (f32, bool)> = areas
            .iter()
            .map(|(&k, a)| {
                let own = a.world_per_uv();
                let per_uv = own.map_or(average, |w| baked.texel_world_of(w));
                (k, (MIN_TEXELS * per_uv, own.is_some()))
            })
            .collect();
        let per_tri: Vec<f32> = islands.iter().map(|k| reach.max(floors[k].0)).collect();
        let raised = baked.settings.curvature_radius.and_then(|requested| {
            let covered = || floors.values().filter(|&&(_, own)| own);
            let up: Vec<f32> = covered().map(|&(f, _)| f).filter(|&f| f > reach).collect();
            (!up.is_empty()).then(|| Raised {
                requested,
                low: up.iter().copied().fold(f32::MAX, f32::min),
                high: up.iter().copied().fold(0.0, f32::max),
                islands: up.len(),
                of: covered().count(),
            })
        });
        Self {
            per_tri,
            reach,
            raised,
        }
    }
}

impl fmt::Display for Raised {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let (low, high) = (sig(self.low), sig(self.high));
        let effective = if low == high {
            low
        } else {
            format!("{low}..{high}")
        };
        write!(
            f,
            "curvature_radius {} is narrower than three texels on {} of {} UV islands, \
             so they bake at {effective}; raise the resolution or give those islands \
             more UV space to keep it",
            sig(self.requested),
            self.islands,
            self.of,
        )
    }
}

pub(super) fn sig(x: f32) -> String {
    if x == 0.0 || !x.is_finite() {
        return format!("{x}");
    }
    let x = f64::from(x);
    let exponent = |v: f64| v.abs().log10().floor() as i32;
    let scale = 10f64.powi(2 - exponent(x));
    let rounded = (x * scale).round() / scale;
    let digits = (2 - exponent(rounded)).max(0) as usize;
    format!("{rounded:.digits$}")
}
