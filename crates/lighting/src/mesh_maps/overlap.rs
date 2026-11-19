//! Texels several triangles share in UV0 (#2310): a wall and the panel screwed on in
//! front of it, a box's front and its hidden back. Each such texel takes the
//! claimant a viewer sees, the **most open** one: the highest ambient occlusion at
//! its own point, traced exactly as the `ao` map traces the texel (its ray count,
//! its stream), with ties to the lower triangle index. Every map then bakes from
//! that one point, so the maps of a bake agree texel for texel, and the pick is the
//! same whichever maps are asked: a bake without `ao` still traces it, on the
//! shared texels only. A layout with no overlap has no claimant and pays nothing.

use super::margin::per_texel;
use super::rays::MeshRays;
use super::Baked;
use crate::lightmap::TexelPoint;

/// Replace each shared texel's point in `baked` by its most open claimant's.
/// `others` are the walk's claimants, (the texel's place in the points, a point),
/// grouped by place in walk order; `rays` traces the model.
pub(super) fn resolve(baked: &mut Baked, others: &[(u32, TexelPoint)], rays: &MeshRays) {
    let mut slots: Vec<u32> = others.iter().map(|&(slot, _)| slot).collect();
    slots.sort_unstable();
    slots.dedup();
    // Every claimant of every shared texel, its owner included: (place, point).
    let mut claims: Vec<(u32, TexelPoint)> = slots
        .iter()
        .map(|&s| (s, baked.points[s as usize]))
        .chain(others.iter().copied())
        .collect();
    claims.sort_by_key(|&(slot, p)| (slot, p.first));
    let points: Vec<TexelPoint> = claims.iter().map(|&(_, p)| p).collect();
    let open = per_texel(&points, |k, p| rays.ao(baked, claims[k].0 as usize, p));
    // Sorted by triangle within a texel: only a strictly more open later claimant
    // takes it, so a tie stays with the lower index.
    let mut best: Vec<usize> = Vec::with_capacity(slots.len());
    for k in 0..claims.len() {
        match best.last_mut() {
            Some(b) if claims[*b].0 == claims[k].0 => {
                if open[k].x > open[*b].x {
                    *b = k;
                }
            }
            _ => best.push(k),
        }
    }
    for k in best {
        let (slot, p) = claims[k];
        baked.points[slot as usize] = p;
    }
}
