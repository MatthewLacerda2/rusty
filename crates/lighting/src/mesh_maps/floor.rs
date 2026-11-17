//! The curvature band's narrowest reach, per UV island (#2292): three of the
//! island's **own** texels, its world size per texel at the bake's resolution, so a
//! band never shrinks below what its texels can show. An island packed small in UV
//! (faces the player never sees) gets a wider floor; the faces given room keep the
//! radius asked for. A radius at or above every island's floor is used as asked.
//!
//! An island whose UVs enclose no area takes the mesh's average texel instead: it
//! covers next to no texel, so its floor only keeps it from reading nothing.

use std::fmt;

use super::Baked;
use crate::lightmap::Areas;
use rusty_core::collections::Map;

/// The narrowest band, in the island's texels.
pub(super) const MIN_TEXELS: f32 = 3.0;

/// The band's radius for each triangle, the widest of them, and what the floor
/// raised.
pub(super) struct Radii {
    pub per_tri: Vec<f32>,
    pub widest: f32,
    pub raised: Option<Raised>,
}

/// The UV islands whose floor raised the `curvature_radius` a bake named: the
/// radius asked for, the narrowest and widest radius those islands baked at, how
/// many were raised and of how many. A bake that names no radius reports none.
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
        let floor = |a: &Areas| {
            MIN_TEXELS
                * a.world_per_uv()
                    .map_or(average, |w| baked.texel_world_of(w))
        };
        let floors: Map<u32, f32> = areas.iter().map(|(&k, a)| (k, floor(a))).collect();
        let per_tri: Vec<f32> = islands.iter().map(|k| reach.max(floors[k])).collect();
        let widest = per_tri.iter().copied().fold(reach, f32::max);
        let raised = baked.settings.curvature_radius.and_then(|requested| {
            let up: Vec<f32> = floors.values().copied().filter(|&f| f > reach).collect();
            (!up.is_empty()).then(|| Raised {
                requested,
                low: up.iter().copied().fold(f32::MAX, f32::min),
                high: up.iter().copied().fold(0.0, f32::max),
                islands: up.len(),
                of: floors.len(),
            })
        });
        Self {
            per_tri,
            widest,
            raised,
        }
    }
}

impl fmt::Display for Raised {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let effective = if self.low == self.high {
            sig(self.low)
        } else {
            format!("{}..{}", sig(self.low), sig(self.high))
        };
        write!(
            f,
            "curvature_radius {} is narrower than three texels on {} of {} UV islands, \
             so they bake at {effective}; raise the resolution or give those islands \
             more UV space to keep it",
            self.requested, self.islands, self.of,
        )
    }
}

/// `x` to three significant figures.
fn sig(x: f32) -> String {
    let digits = (2 - x.abs().log10().floor() as i32).max(0) as usize;
    format!("{x:.digits$}")
}
