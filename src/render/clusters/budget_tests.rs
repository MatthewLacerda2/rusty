//! The light budget (#834): the camera keeps the lights that contribute most to the
//! view, each cluster the ones that contribute most to it, and the cut is the same
//! every time for the same input.

use glam::{Quat, Vec3};

use super::bin::{bin_spheres, Binned, Budget};
use super::grid::AabbCache;
use super::*;
use crate::components::{LightComponent, TransformComponent};
use crate::scene::{Camera, LightType};

fn grid() -> ClusterGrid {
    ClusterGrid::new(&Camera::new(Vec3::ZERO, -90.0, 0.0), 16.0 / 9.0)
}

/// A white point light at `at`.
fn light(at: Vec3, range: f32, intensity: f32) -> LocalLight {
    let transform = TransformComponent {
        position: at,
        rotation: Quat::IDENTITY,
        ..Default::default()
    };
    let light = LightComponent {
        light_type: LightType::Point,
        color: Vec3::ONE,
        intensity,
        range,
        inner_cone: 20.0,
        outer_cone: 30.0,
        cast_shadows: false,
        mode: Default::default(),
    };
    LocalLight::new(&transform, &light).expect("a local light")
}

fn bin_lights(lights: &[LocalLight]) -> Binned {
    bin(&grid(), lights, &mut AabbCache::default())
}

fn bin_with(spheres: &[(Vec3, f32)], budget: &Budget) -> Binned {
    bin_spheres(&grid(), spheres, budget, &mut AabbCache::default())
}

/// Every non-empty cluster list.
fn lists(binned: &Binned) -> impl Iterator<Item = &[u32]> {
    binned
        .ranges
        .iter()
        .filter(|r| r[1] > 0)
        .map(|&[offset, count]| &binned.indices[offset as usize..(offset + count) as usize])
}

#[test]
fn the_brighter_far_light_beats_the_dimmer_near_one() {
    // A dim lamp beside the camera, a bright one lighting the far end of the room.
    let near = light(Vec3::new(0.0, 0.0, -2.0), 1.5, 0.5);
    let far = light(Vec3::new(0.0, 0.0, -15.0), 8.0, 8.0);
    let spheres = [near.sphere(), far.sphere()];
    let brightness = [near.brightness(), far.brightness()];
    let budget = Budget {
        per_camera: 1,
        per_cluster: usize::MAX,
        brightness: Some(&brightness),
    };
    let binned = bin_with(&spheres, &budget);
    assert_eq!((binned.visible, binned.dropped), (1, 1));
    assert!(binned.indices.iter().all(|&i| i == 1), "the bright stays");
    // Nearest-surface ranking (the decals' budget) would have kept the near one.
    let nearest = bin_with(&spheres, &Budget::nearest(1));
    assert!(nearest.indices.iter().all(|&i| i == 0));
}

#[test]
fn past_the_camera_budget_a_bright_far_light_outranks_dim_near_ones() {
    // A grid of dim, tight lights near the camera fills the budget exactly.
    let mut lights: Vec<_> = (0..MAX_VISIBLE_LIGHTS)
        .map(|i| {
            let (x, y) = ((i % 8) as f32 - 3.5, (i / 8) as f32 - 3.5);
            light(Vec3::new(x * 0.5, y * 0.5, -6.0), 0.3, 0.5)
        })
        .collect();
    lights.push(light(Vec3::new(0.0, 0.0, -20.0), 2.0, 8.0));
    let binned = bin_lights(&lights);
    assert_eq!(binned.visible as usize, MAX_VISIBLE_LIGHTS);
    assert_eq!(binned.dropped, 1);
    let bright = MAX_VISIBLE_LIGHTS as u32;
    assert!(
        binned.indices.contains(&bright),
        "the bright far light shades"
    );
}

#[test]
fn cluster_lists_never_exceed_the_cluster_budget() {
    // 40 lights on one spot reach the same clusters; the brightest 32 win in each.
    let lights: Vec<_> = (0..40)
        .map(|i| light(Vec3::new(0.0, 0.0, -6.0), 3.0, 1.0 + i as f32))
        .collect();
    let binned = bin_lights(&lights);
    assert_eq!((binned.visible, binned.dropped), (40, 0));
    let brightest: Vec<u32> = (8..40).collect();
    let mut clusters = 0;
    for list in lists(&binned) {
        assert_eq!(
            list,
            brightest.as_slice(),
            "each cluster keeps the brightest"
        );
        clusters += 1;
    }
    assert!(clusters > 0);
    assert_eq!(binned.cluster_dropped as usize, clusters * 8);
}

#[test]
fn within_a_cluster_the_light_reaching_in_brightest_wins() {
    // Equal lights: the one whose centre sits in the cluster beats ones that only
    // graze it, so a crowded corner keeps its nearest lamps.
    let mut lights: Vec<_> = (0..MAX_CLUSTER_LIGHTS)
        .map(|_| light(Vec3::new(0.0, 0.0, -9.0), 3.0, 1.0))
        .collect();
    lights.push(light(Vec3::new(0.0, 0.0, -6.0), 3.0, 1.0));
    let binned = bin_lights(&lights);
    let grid = grid();
    let view_proj = Camera::new(Vec3::ZERO, -90.0, 0.0).build_view_projection(16.0 / 9.0);
    let cluster = grid.cluster_at(view_proj, Vec3::new(0.0, 0.0, -6.0));
    let [offset, count] = binned.ranges[cluster as usize];
    let list = &binned.indices[offset as usize..(offset + count) as usize];
    assert!(list.len() <= MAX_CLUSTER_LIGHTS);
    assert!(list.contains(&(MAX_CLUSTER_LIGHTS as u32)), "{list:?}");
}

#[test]
fn the_same_input_gives_the_same_cut() {
    // Identical lights tie everywhere: the index decides, so the lowest stay.
    let lights: Vec<_> = (0..MAX_VISIBLE_LIGHTS + 16)
        .map(|_| light(Vec3::new(1.0, 0.5, -8.0), 4.0, 2.0))
        .collect();
    let (a, b) = (bin_lights(&lights), bin_lights(&lights));
    assert_eq!((&a.ranges, &a.indices), (&b.ranges, &b.indices));
    assert_eq!(
        (a.dropped, a.cluster_dropped),
        (b.dropped, b.cluster_dropped)
    );
    let lowest: Vec<u32> = (0..MAX_CLUSTER_LIGHTS as u32).collect();
    assert!(lists(&a).all(|list| list == lowest.as_slice()));
}
