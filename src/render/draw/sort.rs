//! src/render/draw/sort.rs — the one back-to-front key every blended pass sorts by.
//!
//! Translucent solids (#242), particle emitters and the particles inside an
//! alpha-blended emitter (#440) all order by view depth — the distance along the
//! camera's forward axis — so two passes can never disagree about what is in front.
//! World-space UI canvases (#429) should sort with this key too.

use glam::Vec3;

/// `pos`'s depth in front of the camera, along its forward axis.
pub(crate) fn view_depth(pos: Vec3, cam_pos: Vec3, cam_fwd: Vec3) -> f32 {
    (pos - cam_pos).dot(cam_fwd)
}

/// Order `items` (each paired with its [`view_depth`]) farthest first, so nearer
/// blended surfaces draw over farther ones. Stable: equal depths keep their order.
pub(crate) fn back_to_front<T>(items: &mut [(T, f32)]) {
    items.sort_by(|a, b| b.1.total_cmp(&a.1));
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
