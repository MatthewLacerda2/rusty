//! Texels several triangles share in UV0 (#2310): a wall and the panel screwed on in
//! front of it, a box's front and its hidden back. Each such texel takes the
//! claimant a viewer sees, the **most open** one: the highest ambient occlusion at
//! its own point, traced exactly as the `ao` map traces the texel (its ray count,
//! its stream), with ties to the lower triangle index. Every map then bakes from
//! that one point, so the maps of a bake agree texel for texel, and the pick is the
//! same whichever maps are asked: a bake without `ao` still traces it, on the
//! shared texels only. A layout with no overlap has no claimant and pays nothing.

use super::margin::{per_texel, BLOCK};
use super::rays::MeshRays;
use super::Baked;
use crate::lightmap::TexelPoint;

/// Replace each shared texel's point in `baked` by its most open claimant's.
/// `claims` are the walk's claimants, (the texel's place in the points, a point);
/// the owners join them in place, so the list is held once. Openness is traced a
/// [`BLOCK`] of claims at a time, one `f32` each kept; `rays` traces the model.
pub(super) fn resolve(baked: &mut Baked, mut claims: Vec<(u32, TexelPoint)>, rays: &MeshRays) {
    let mut slots: Vec<u32> = claims.iter().map(|&(slot, _)| slot).collect();
    slots.sort_unstable();
    slots.dedup();
    claims.extend(slots.iter().map(|&s| (s, baked.points[s as usize])));
    drop(slots);
    claims.sort_by_key(|&(slot, p)| (slot, p.first));
    let mut open: Vec<f32> = Vec::with_capacity(claims.len());
    for (b, block) in claims.chunks(BLOCK).enumerate() {
        let points: Vec<TexelPoint> = block.iter().map(|&(_, p)| p).collect();
        let slot = |k: usize| claims[b * BLOCK + k].0 as usize;
        open.extend(
            per_texel(&points, |k, p| rays.ao(baked, slot(k), p))
                .iter()
                .map(|v| v.x),
        );
    }
    // Sorted by triangle within a texel: only a strictly more open later claimant
    // takes it, so a tie stays with the lower index.
    let mut best: Option<usize> = None;
    for k in 0..claims.len() {
        match best {
            Some(b) if claims[b].0 == claims[k].0 => {
                if open[k] > open[b] {
                    best = Some(k);
                }
            }
            _ => {
                if let Some(b) = best {
                    baked.points[claims[b].0 as usize] = claims[b].1;
                }
                best = Some(k);
            }
        }
    }
    if let Some(b) = best {
        baked.points[claims[b].0 as usize] = claims[b].1;
    }
}
