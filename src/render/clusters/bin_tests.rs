//! CPU light binning (#434): lights land in the clusters the shader will look up,
//! lights out of view cost nothing, and the per-camera budget is reported.

use glam::{Quat, Vec3};

use super::bin::Binned;
use super::*;
use crate::components::{LightComponent, Projection, TransformComponent};
use crate::scene::{Camera, LightType};

/// A camera at the origin looking down -Z (yaw -90°), 16:9.
fn camera() -> Camera {
    Camera::new(Vec3::ZERO, -90.0, 0.0)
}

fn light(kind: LightType, at: Vec3, range: f32) -> LocalLight {
    let transform = TransformComponent {
        position: at,
        rotation: Quat::IDENTITY,
        ..Default::default()
    };
    let light = LightComponent {
        light_type: kind,
        color: Vec3::ONE,
        intensity: 1.0,
        range,
        inner_cone: 20.0,
        outer_cone: 30.0,
    };
    LocalLight::new(&transform, &light).expect("a local light")
}

fn lists_of(binned: &Binned, cluster: u32) -> &[u32] {
    let [offset, count] = binned.ranges[cluster as usize];
    &binned.indices[offset as usize..(offset + count) as usize]
}

/// Every sampled point within a light's range and inside the view finds that light
/// in the cluster the shader picks for it — the binning is conservative.
fn assert_conservative(cam: &Camera, lights: &[LocalLight]) {
    let aspect = 16.0 / 9.0;
    let grid = ClusterGrid::new(cam, aspect);
    let view_proj = cam.build_view_projection(aspect);
    let frustum = crate::render::Frustum::from_view_proj(view_proj);
    let binned = bin(&grid, lights);
    for (n, l) in lights.iter().enumerate() {
        let (c, r) = l.sphere();
        let steps = 6;
        for x in 0..=steps {
            for y in 0..=steps {
                for z in 0..=steps {
                    let t = Vec3::new(x as f32, y as f32, z as f32) / steps as f32 * 2.0 - 1.0;
                    let p = c + t * r * 0.999;
                    if p.distance(c) > r || !frustum.intersects_aabb(p, p) {
                        continue;
                    }
                    let cluster = grid.cluster_at(view_proj, p);
                    assert!(
                        lists_of(&binned, cluster).contains(&(n as u32)),
                        "light {n} missing from cluster {cluster} at {p}"
                    );
                }
            }
        }
    }
}

#[test]
fn lights_land_in_every_cluster_they_reach() {
    let lights = [
        light(LightType::Point, Vec3::new(0.0, 0.0, -5.0), 3.0),
        light(LightType::Point, Vec3::new(4.0, -1.0, -20.0), 6.0),
        // Straddling the near plane and the left edge of the view.
        light(LightType::Spotlight, Vec3::new(-1.0, 0.5, 0.5), 2.0),
        light(LightType::Point, Vec3::new(-30.0, 10.0, -60.0), 25.0),
    ];
    assert_conservative(&camera(), &lights);
    let mut ortho = camera();
    ortho.projection = Projection::Orthographic { size: 8.0 };
    assert_conservative(&ortho, &lights);
}

#[test]
fn lights_out_of_view_are_culled_and_cost_nothing() {
    let lights = [
        light(LightType::Point, Vec3::new(0.0, 0.0, 10.0), 3.0), // behind
        light(LightType::Point, Vec3::new(0.0, 0.0, -500.0), 3.0), // past far
        light(LightType::Point, Vec3::new(60.0, 0.0, -10.0), 3.0), // off to the side
        light(LightType::Point, Vec3::new(0.0, 0.0, -5.0), 0.0), // no reach
    ];
    let binned = bin(&ClusterGrid::new(&camera(), 16.0 / 9.0), &lights);
    assert_eq!((binned.visible, binned.culled, binned.dropped), (0, 4, 0));
    assert!(binned.indices.is_empty());
}

#[test]
fn ranges_partition_the_index_list() {
    let lights: Vec<_> = (0..12)
        .map(|i| light(LightType::Point, Vec3::new(i as f32 - 6.0, 0.0, -8.0), 2.0))
        .collect();
    let binned = bin(&ClusterGrid::new(&camera(), 16.0 / 9.0), &lights);
    assert_eq!(binned.visible, 12);
    let mut next = 0;
    for &[offset, count] in &binned.ranges {
        assert_eq!(offset, next);
        next += count;
    }
    assert_eq!(next as usize, binned.indices.len());
}

#[test]
fn past_the_budget_the_nearest_lights_win_and_the_rest_are_dropped() {
    let extra = 40;
    let lights: Vec<_> = (0..MAX_VISIBLE_LIGHTS + extra)
        .map(|i| {
            light(
                LightType::Point,
                Vec3::new(0.0, 0.0, -2.0 - i as f32 * 0.5),
                0.4,
            )
        })
        .collect();
    let binned = bin(&ClusterGrid::new(&camera(), 16.0 / 9.0), &lights);
    assert_eq!(binned.visible as usize, MAX_VISIBLE_LIGHTS);
    assert_eq!(binned.dropped as usize, extra);
    let max = *binned.indices.iter().max().unwrap() as usize;
    assert_eq!(
        max,
        MAX_VISIBLE_LIGHTS - 1,
        "the farthest lights were the ones cut"
    );
}
