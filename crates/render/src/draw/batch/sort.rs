use std::ops::Range;

use glam::Vec3;

use super::{BatchKey, DrawItem};

pub(crate) fn view_depth(pos: Vec3, cam_pos: Vec3, cam_fwd: Vec3) -> f32 {
    (pos - cam_pos).dot(cam_fwd)
}

/// Order `items` (each paired with its [`view_depth`]) farthest first, so nearer
/// blended surfaces draw over farther ones. Stable: equal depths keep their order.
pub(crate) fn back_to_front<T>(items: &mut [(T, f32)]) {
    items.sort_by(|a, b| b.1.total_cmp(&a.1));
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct TransparentOrder {
    pub priority: i32,
    pub depth: f32,
}

/// HDRP: a lower priority draws first, so a higher one lands on top whatever its depth.
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

const NEAREST: f32 = 0.05;

pub(crate) fn depth_bucket(depth: f32) -> i32 {
    // `max` returns NEAREST for a NaN depth too.
    (depth.max(NEAREST).log2() * BUCKETS_PER_OCTAVE).floor() as i32
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum OpaqueOrder {
    #[default]
    Unranked,
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

    pub(super) fn rank(self, key: &BatchKey) -> u8 {
        match self {
            Self::Unranked => 0,
            Self::OwnDepthFirst => u8::from(key.in_prepass()),
        }
    }
}

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
