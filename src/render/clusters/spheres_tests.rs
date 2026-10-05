//! Binning bare spheres (#638, the decals' path) and the range list the shader
//! reads: the cull tally counts only what left the view, the budget keeps the
//! nearest *surfaces*, and the decal ranges follow the lights', shifted past their
//! indices.

use glam::Vec3;

use super::bin::{bin_spheres, Binned, Budget};
use super::gpu::concat;
use super::grid::AabbCache;
use super::{ClusterGrid, CLUSTER_COUNT};
use crate::scene::Camera;

/// Bin `spheres` for a camera at the origin looking down -Z, keeping `budget`.
fn bin(spheres: &[(Vec3, f32)], budget: usize) -> Binned {
    let grid = ClusterGrid::new(&Camera::new(Vec3::ZERO, -90.0, 0.0), 16.0 / 9.0);
    bin_spheres(
        &grid,
        spheres,
        &Budget::nearest(budget),
        &mut AabbCache::default(),
    )
}

/// A sphere `distance` straight ahead.
fn ahead(distance: f32, radius: f32) -> (Vec3, f32) {
    (Vec3::new(0.0, 0.0, -distance), radius)
}

#[test]
fn only_spheres_out_of_view_count_as_culled() {
    let behind = (Vec3::new(0.0, 0.0, 10.0), 1.0);
    let binned = bin(&[ahead(5.0, 1.0), behind, ahead(8.0, 1.0)], 8);
    assert_eq!((binned.visible, binned.culled, binned.dropped), (2, 1, 0));
}

#[test]
fn the_budget_keeps_the_nearest_surfaces() {
    // A big sphere farther out whose surface is nearer than a small close one's.
    let binned = bin(&[ahead(10.0, 4.0), ahead(7.0, 0.5)], 1);
    assert_eq!((binned.visible, binned.dropped), (1, 1));
    assert!(
        binned.indices.iter().all(|&i| i == 0),
        "surface at 6 beats 6.5"
    );
    // And a small close one beats a big far one: distance, not proportion.
    let binned = bin(&[ahead(10.0, 2.0), ahead(3.0, 0.5)], 1);
    assert!(
        binned.indices.iter().all(|&i| i == 1),
        "surface at 2.5 beats 8"
    );
}

#[test]
fn decal_ranges_follow_the_lights_shifted_past_their_indices() {
    let lights = bin(&[ahead(5.0, 1.0), ahead(9.0, 1.0)], 8);
    let decals = bin(&[ahead(6.0, 0.5)], 8);
    let (ranges, indices) = concat(&lights, &decals);
    assert_eq!(ranges.len(), 2 * CLUSTER_COUNT);
    assert_eq!(&ranges[..CLUSTER_COUNT], lights.ranges.as_slice());
    let base = lights.indices.len();
    assert_eq!(&indices[..base], lights.indices.as_slice());
    assert_eq!(&indices[base..], decals.indices.as_slice());
    for (cluster, &[offset, count]) in ranges[CLUSTER_COUNT..].iter().enumerate() {
        let own = decals.ranges[cluster];
        assert_eq!((offset, count), (own[0] + base as u32, own[1]));
    }
    assert!(!decals.indices.is_empty(), "the decal landed somewhere");
}
