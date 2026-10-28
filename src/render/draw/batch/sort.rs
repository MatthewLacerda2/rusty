//! The view-depth orders the solid passes draw in.
//!
//! Translucent solids (#242), particle emitters and the particles inside an
//! alpha-blended emitter (#440) all order back to front by view depth — the distance
//! along the camera's forward axis — so two passes can never disagree about what is
//! in front. World-space UI canvases (#429) should sort with this key too.
//!
//! Opaque solids order roughly front to back instead (#1499), as Unity's
//! `SortingCriteria.CommonOpaque` does (its `QuantizedFrontToBack`): the nearest
//! surfaces fill the depth buffer first, so the GPU's early depth test skips the
//! fragment shader of what they hide. The depth is quantized into buckets, so batches
//! at near-equal depths keep their key (pipeline, mesh, material) order and the state
//! changes stay few.

use std::ops::Range;

use glam::Vec3;

use super::DrawItem;

/// `pos`'s depth in front of the camera, along its forward axis.
pub(crate) fn view_depth(pos: Vec3, cam_pos: Vec3, cam_fwd: Vec3) -> f32 {
    (pos - cam_pos).dot(cam_fwd)
}

/// Order `items` (each paired with its [`view_depth`]) farthest first, so nearer
/// blended surfaces draw over farther ones. Stable: equal depths keep their order.
pub(crate) fn back_to_front<T>(items: &mut [(T, f32)]) {
    items.sort_by(|a, b| b.1.total_cmp(&a.1));
}

/// Buckets per doubling of depth: each spans about 19% of its distance, so the
/// order is finer near the camera, where a surface covers the most pixels.
const BUCKETS_PER_OCTAVE: f32 = 4.0;

/// The depth every nearer one (a solid whose centre is beside or behind the camera)
/// counts as: they share the first bucket.
const NEAREST: f32 = 0.05;

/// `depth`'s front-to-back bucket: a solid in a lower bucket draws first.
pub(crate) fn depth_bucket(depth: f32) -> i32 {
    // `max` returns NEAREST for a NaN depth too.
    (depth.max(NEAREST).log2() * BUCKETS_PER_OCTAVE).floor() as i32
}

/// Order opaque `items` for instancing and early depth rejection (#470, #1499):
/// grouped into runs of equal keys, so each run is one draw, its copies nearest
/// first (an instanced draw rasterizes in instance order), then the runs nearest
/// first by their nearest item's [`depth_bucket`]. Stable: runs in one bucket keep
/// their key order.
pub(crate) fn front_to_back(mut items: Vec<DrawItem>) -> Vec<DrawItem> {
    items.sort_by(|a, b| a.key.cmp(&b.key).then(a.depth.total_cmp(&b.depth)));
    let mut runs: Vec<(i32, Range<usize>)> = Vec::new();
    for (i, item) in items.iter().enumerate() {
        let bucket = depth_bucket(item.depth);
        match runs.last_mut() {
            Some((nearest, run)) if items[run.start].key == item.key => {
                *nearest = (*nearest).min(bucket);
                run.end = i + 1;
            }
            _ => runs.push((bucket, i..i + 1)),
        }
    }
    runs.sort_by_key(|&(bucket, _)| bucket);
    let mut slots: Vec<Option<DrawItem>> = items.into_iter().map(Some).collect();
    runs.into_iter()
        .flat_map(|(_, run)| run)
        .map(|i| slots[i].take().expect("each item is in exactly one run"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn farthest_first_along_the_view_axis() {
        let (eye, fwd) = (Vec3::ZERO, Vec3::NEG_Z);
        let mut items: Vec<(&str, f32)> = [("near", -1.0), ("far", -9.0), ("mid", -4.0)]
            .into_iter()
            .map(|(name, z)| (name, view_depth(Vec3::new(3.0, 0.0, z), eye, fwd)))
            .collect();
        back_to_front(&mut items);
        let order: Vec<_> = items.iter().map(|i| i.0).collect();
        assert_eq!(order, ["far", "mid", "near"]);
    }
}
