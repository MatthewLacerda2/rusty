//! A cluster ranks its lights by what they deliver across it (#1699): a dim lamp
//! inside the cluster's box does not outrank a bright one a few metres off, which
//! the windowed falloff's bare `1/d²` (its `ε²` floor alone) would let it do.

use glam::Vec3;

use super::bin::{bin_spheres, Budget};
use super::grid::AabbCache;
use super::*;
use crate::components::LightComponent;
use crate::scene::Camera;

fn light(at: Vec3, range: f32, intensity: f32) -> LocalLight {
    let light = LightComponent {
        intensity,
        range,
        ..crate::scene::authoring::default_light()
    };
    LocalLight::new(at, Vec3::NEG_Z, &light).expect("a local light")
}

#[test]
fn a_candle_inside_a_cluster_does_not_outrank_a_floodlight_beside_it() {
    let grid = ClusterGrid::new(&Camera::new(Vec3::ZERO, -90.0, 0.0), 16.0 / 9.0);
    let candle = light(Vec3::new(0.0, 0.0, -6.0), 0.5, 0.2);
    let flood = light(Vec3::new(5.0, 0.0, -6.0), 12.0, 100.0);
    let spheres = [candle.sphere(), flood.sphere()];
    let brightness = [candle.brightness(), flood.brightness()];
    let budget = |per_cluster| Budget {
        per_camera: usize::MAX,
        per_cluster,
        brightness: Some(&brightness),
    };
    let all = bin_spheres(
        &grid,
        &spheres,
        &budget(usize::MAX),
        &mut AabbCache::default(),
    );
    let capped = bin_spheres(&grid, &spheres, &budget(1), &mut AabbCache::default());
    let list = |b: &super::bin::Binned, c: usize| {
        let [offset, count] = b.ranges[c];
        let mut l = b.indices[offset as usize..(offset + count) as usize].to_vec();
        l.sort_unstable();
        l
    };
    let shared: Vec<usize> = (0..all.ranges.len())
        .filter(|&c| list(&all, c) == [0, 1])
        .collect();
    assert!(
        !shared.is_empty(),
        "the candle's clusters are the floodlight's too"
    );
    for c in shared {
        assert_eq!(list(&capped, c), [1], "cluster {c} keeps the floodlight");
    }
}
