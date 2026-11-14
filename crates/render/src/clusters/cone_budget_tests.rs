//! A coned spot under the light budget (#1712, #834): the cone decides only which
//! clusters a kept spot lists in, never which lights the camera keeps, and a
//! cluster ranks a spot by its distance from the lamp, not from its cone's centre.

use glam::Vec3;

use super::bin::shadow_requests;
use super::bin_tests::lists_of;
use super::grid::AabbCache;
use super::*;
use rusty_components::LightComponent;
use rusty_scene::{Camera, LightType};

fn camera() -> Camera {
    Camera::new(Vec3::ZERO, -90.0, 0.0)
}

fn grid() -> ClusterGrid {
    ClusterGrid::new(&camera(), 16.0 / 9.0)
}

fn light(kind: LightType, at: Vec3, range: f32, intensity: f32) -> LocalLight {
    let light = LightComponent {
        light_type: kind,
        intensity,
        range,
        inner_cone: 6.0,
        outer_cone: 12.0,
        ..rusty_scene::authoring::default_light()
    };
    LocalLight::new(at, Vec3::NEG_Z, &light).expect("a local light")
}

/// A 12°, 50 m beacon just ahead of the camera, shining away from it down the
/// view: its cone's bounding sphere sits about 25 m down the beam.
fn beacon() -> LocalLight {
    light(LightType::Spotlight, Vec3::new(0.0, 0.0, -5.0), 50.0, 1.0)
}

/// The indices any cluster lists.
fn kept(binned: &super::bin::Binned) -> Vec<u32> {
    let mut v = binned.indices.clone();
    v.sort_unstable();
    v.dedup();
    v
}

#[test]
fn past_the_camera_budget_a_spot_is_kept_by_its_range_sphere() {
    // 64 point lights the camera sits inside: each scores ~0.74 over the view, the
    // beacon ~0.99 by its range sphere (~0.41 by its cone's sphere). One must go.
    let mut lights: Vec<_> = (0..MAX_VISIBLE_LIGHTS)
        .map(|i| {
            let x = i as f32 * 0.1 - 3.0;
            light(LightType::Point, Vec3::new(x, 0.0, -6.0), 10.0, 1.0)
        })
        .collect();
    lights.push(beacon());
    let spot = MAX_VISIBLE_LIGHTS as u32;
    let binned = bin(&grid(), &lights, &mut AabbCache::default());
    assert_eq!(binned.dropped, 1);
    assert!(kept(&binned).contains(&spot), "the beacon is kept");
    // The shadow atlas's cut keeps the very same lights.
    let asks: Vec<_> = lights.iter().map(|&l| (l, true)).collect();
    let wanted: Vec<u32> = (0..lights.len() as u32)
        .filter(|&i| shadow_requests(&grid(), &asks)[i as usize].1)
        .collect();
    assert_eq!(wanted, kept(&binned));
}

#[test]
fn past_the_cluster_budget_a_spot_beside_its_lamp_is_kept() {
    // 32 point lights crowd the beam just past the lamp; each grazes the cluster a
    // metre in front of it from 3 m away. The beacon's lamp is right there.
    let mut lights: Vec<_> = (0..MAX_CLUSTER_LIGHTS)
        .map(|_| light(LightType::Point, Vec3::new(0.0, 0.0, -9.0), 4.0, 1.0))
        .collect();
    lights.push(beacon());
    let binned = bin(&grid(), &lights, &mut AabbCache::default());
    let view_proj = camera().build_view_projection(16.0 / 9.0);
    let cluster = grid().cluster_at(view_proj, Vec3::new(0.0, 0.0, -6.0));
    let list = lists_of(&binned, cluster);
    assert_eq!(list.len(), MAX_CLUSTER_LIGHTS);
    assert!(list.contains(&(MAX_CLUSTER_LIGHTS as u32)), "{list:?}");
    assert!(binned.cluster_dropped > 0, "the cluster was over budget");
}
