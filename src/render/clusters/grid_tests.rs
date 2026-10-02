//! The cluster-box cache (#434): keyed by lens, so moving a camera rebuilds nothing.

use glam::Vec3;

use super::{AabbCache, ClusterGrid};
use crate::scene::Camera;

fn camera() -> Camera {
    Camera::new(Vec3::ZERO, -90.0, 0.0)
}

#[test]
fn cluster_boxes_are_cached_per_lens_not_per_position() {
    let mut cache = AabbCache::default();
    let first = cache.get(&ClusterGrid::new(&camera(), 16.0 / 9.0)).as_ptr();
    let mut moved = camera();
    moved.position = Vec3::new(3.0, 1.0, -2.0);
    let again = cache.get(&ClusterGrid::new(&moved, 16.0 / 9.0)).as_ptr();
    assert_eq!(
        first, again,
        "a moved camera with the same lens reuses its boxes"
    );
    let mut zoomed = camera();
    zoomed.fov = 70.0;
    let other = cache.get(&ClusterGrid::new(&zoomed, 16.0 / 9.0)).as_ptr();
    assert_ne!(first, other);
}
