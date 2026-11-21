//! The view-depth orders the solid passes draw in.
//!
//! Translucent solids (#242), particle emitters and the particles inside an
//! alpha-blended emitter (#440) all order back to front by view depth — the distance
//! along the camera's forward axis — so two passes can never disagree about what is
//! in front. Translucent solids sort by their material's Sorting Priority first
//! ([`transparent_order`], #2039), as HDRP's transparent queue does; particles
//! and ribbons have no priority and sort by depth alone. World-space UI canvases (#429) should sort with this key too.
//!
//! Opaque solids order roughly front to back instead (#1499), as Unity's
//! `SortingCriteria.CommonOpaque` does (its `QuantizedFrontToBack`): the nearest
//! surfaces fill the depth buffer first, so the GPU's early depth test skips the
//! fragment shader of what they hide. The depth is quantized into buckets, so batches
//! at near-equal depths keep their key (pipeline, mesh, material) order and the state
//! changes stay few.
//!
//! Over a primed depth one rank comes ahead of the depth ([`OpaqueOrder`], #2484):
//! a lit solid there tests the prepass's depth and writes none, so the solids the
//! prepass left out, which write their own depth (an unlit solid: a light's own
//! mesh, a gizmo solid), draw first, and the lit pixels they cover are rejected
//! rather than shaded and then painted over.

use std::ops::Range;

use glam::Vec3;

use super::{BatchKey, DrawItem};

/// `pos`'s depth in front of the camera, along its forward axis.
pub(crate) fn view_depth(pos: Vec3, cam_pos: Vec3, cam_fwd: Vec3) -> f32 {
    (pos - cam_pos).dot(cam_fwd)
}

/// Order `items` (each paired with its [`view_depth`]) farthest first, so nearer
/// blended surfaces draw over farther ones. Stable: equal depths keep their order.
pub(crate) fn back_to_front<T>(items: &mut [(T, f32)]) {
    items.sort_by(|a, b| b.1.total_cmp(&a.1));
}

/// Where a translucent solid draws in the sorted pass (#2039): its material's
/// HDRP *Sorting Priority*, then its [`view_depth`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct TransparentOrder {
    pub priority: i32,
    pub depth: f32,
}

/// Order translucent `items` as HDRP's transparent queue does: by priority, a
/// lower one first (HDRP adds it to the material's render queue, which Unity
/// sorts before depth), so a higher one draws on top whatever its depth; then
/// farthest first ([`back_to_front`]). Stable, so equal keys keep the order they
/// were pushed in (objects in `World::ids_with_mesh` order, an object's slots in
/// slot order, at one shared depth): a fixed input draws the same frame each time.
pub(crate) fn transparent_order<T>(items: &mut [(T, TransparentOrder)]) {
    items.sort_by(|a, b| {
        let (a, b) = (a.1, b.1);
        a.priority
            .cmp(&b.priority)
            .then(b.depth.total_cmp(&a.depth))
    });
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

/// How a camera's opaque runs are ranked ahead of their depth (#2484), decided
/// before they batch by whether its forward pass will be primed
/// (`Renderer::opaque_order`, the same rule as `Renderer::primes`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum OpaqueOrder {
    /// No prepass under the forward pass: every solid writes its depth, so every
    /// run shares one rank and they order by depth alone (with instancing off, as
    /// given).
    #[default]
    Unranked,
    /// A primed forward pass: the runs outside the prepass first
    /// ([`BatchKey::in_prepass`] false; every opaque pipeline such a run draws with
    /// writes depth), then the primed ones, each rank front to back (with
    /// instancing off, each in the order given).
    OwnDepthFirst,
}

impl OpaqueOrder {
    /// The order for a forward pass that is `primed` or not.
    pub(crate) fn primed(primed: bool) -> Self {
        match primed {
            true => Self::OwnDepthFirst,
            false => Self::Unranked,
        }
    }

    /// `key`'s rank: a lower one draws first, whatever its depth. A pure function of
    /// the key, so equal keys (one run) share it and the rank never splits a run.
    pub(super) fn rank(self, key: &BatchKey) -> u8 {
        match self {
            Self::Unranked => 0,
            Self::OwnDepthFirst => u8::from(key.in_prepass()),
        }
    }
}

/// Order opaque `items` for instancing and early depth rejection (#470, #1499):
/// grouped into runs of equal keys, so each run is one draw, its copies nearest
/// first (an instanced draw rasterizes in instance order), then the runs by
/// `order`'s rank, and inside a rank nearest first by their nearest item's
/// [`depth_bucket`]. Stable: runs of one rank and bucket keep their key order.
pub(crate) fn front_to_back(mut items: Vec<DrawItem>, order: OpaqueOrder) -> Vec<DrawItem> {
    items.sort_by(|a, b| a.key.cmp(&b.key).then(a.depth.total_cmp(&b.depth)));
    let mut runs: Vec<((u8, i32), Range<usize>)> = Vec::new();
    for (i, item) in items.iter().enumerate() {
        let bucket = depth_bucket(item.depth);
        match runs.last_mut() {
            Some(((_, nearest), run)) if items[run.start].key == item.key => {
                *nearest = (*nearest).min(bucket);
                run.end = i + 1;
            }
            _ => runs.push(((order.rank(&item.key), bucket), i..i + 1)),
        }
    }
    runs.sort_by_key(|&(at, _)| at);
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

    fn order(items: &[(&'static str, TransparentOrder)]) -> Vec<&'static str> {
        let mut items = items.to_vec();
        transparent_order(&mut items);
        items.iter().map(|i| i.0).collect()
    }

    fn at(priority: i32, depth: f32) -> TransparentOrder {
        TransparentOrder { priority, depth }
    }

    #[test]
    fn priority_orders_before_depth() {
        // A near pane at a lower priority draws before a far one (#2039), so the
        // far one lands on top; at equal priority, farthest first.
        let items = [
            ("near", at(-1, 2.0)),
            ("far", at(0, 9.0)),
            ("mid", at(0, 4.0)),
        ];
        assert_eq!(order(&items), ["near", "far", "mid"]);
        let items = [("high", at(50, 9.0)), ("low", at(-50, 1.0))];
        assert_eq!(order(&items), ["low", "high"]);
    }

    #[test]
    fn equal_keys_keep_the_order_they_came_in() {
        // One object's slots share its depth: at equal priority, slot order holds.
        let items = [
            ("glass", at(0, 5.0)),
            ("liquid", at(0, 5.0)),
            ("cork", at(0, 5.0)),
        ];
        assert_eq!(order(&items), ["glass", "liquid", "cork"]);
        let items = [("liquid", at(0, 5.0)), ("glass", at(0, 5.0))];
        assert_eq!(order(&items), ["liquid", "glass"]);
    }
}
