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
        mode: rusty_components::LightMode::Mixed,
        ..rusty_scene::create::default_light()
    };
    LocalLight::new(at, Vec3::NEG_Z, &light).expect("a local light")
}

fn beacon() -> LocalLight {
    light(LightType::Spotlight, Vec3::new(0.0, 0.0, -5.0), 50.0, 1.0)
}

fn kept(binned: &super::bin::Binned) -> Vec<u32> {
    let mut v = binned.indices.clone();
    v.sort_unstable();
    v.dedup();
    v
}

#[test]
fn past_the_camera_budget_a_spot_is_kept_by_its_range_sphere() {
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
    let asks: Vec<_> = lights.iter().map(|&l| (l, true)).collect();
    let wanted: Vec<u32> = (0..lights.len() as u32)
        .filter(|&i| shadow_requests(&grid(), &asks)[i as usize].1)
        .collect();
    assert_eq!(wanted, kept(&binned));
}

#[test]
fn past_the_cluster_budget_a_spot_beside_its_lamp_is_kept() {
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
