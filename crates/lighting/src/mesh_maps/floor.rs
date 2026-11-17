//! The curvature band's narrowest reach, per UV island (#2292): three of the
//! island's **own** texels, its world size per texel at the bake's resolution, so a
//! band never shrinks below what its texels can show. An island packed small in UV
//! (faces the player never sees) gets a wider floor; the faces given room keep the
//! radius asked for. A radius at or above every island's floor is used as asked.
//!
//! An island whose UVs enclose no area takes the mesh's average texel instead: it
//! covers no texel, so its floor only keeps it from reading nothing, and it is
//! never reported as raised (faces collapsed on purpose need no UV space).

use std::fmt;

use super::Baked;
use crate::lightmap::Areas;
use rusty_core::collections::Map;

/// The narrowest band, in the island's texels.
pub(super) const MIN_TEXELS: f32 = 3.0;

/// The band's radius for each triangle, the reach asked for (the narrowest of
/// them), and what the floor raised.
pub(super) struct Radii {
    pub per_tri: Vec<f32>,
    pub reach: f32,
    pub raised: Option<Raised>,
}

/// The UV islands whose floor raised the `curvature_radius` a bake named: the
/// radius asked for, the narrowest and widest radius those islands baked at, how
/// many were raised and of how many islands that cover texels. A bake that names
/// no radius reports none.
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
        // Per island, its floor and whether its UVs enclose any area.
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

/// `x` to three significant figures, rounded first so a value that rounds up to
/// the next power of ten keeps three (0.0099996 is `0.0100`). Zero and a value
/// that is not finite print as they are.
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
