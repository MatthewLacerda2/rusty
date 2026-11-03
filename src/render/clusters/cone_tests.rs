//! Cone-culled spot binning (#1712): over random spotlights, near the camera and
//! straddling its near plane, a spot's clusters are a subset of its range
//! sphere's and include every cluster its cone reaches, in a perspective and an
//! orthographic view; a cone too wide to trim keeps its range sphere.

use glam::Vec3;

use super::bin::{bin_spheres, Budget};
use super::bin_tests::lists_of;
use super::grid::AabbCache;
use super::*;
use crate::components::{LightComponent, Projection};
use crate::core::random::Random;
use crate::scene::{Camera, LightType};

/// A spotlight at `at` along `dir`, its outer cone `outer` degrees.
fn spot(at: Vec3, dir: Vec3, outer: f32, range: f32) -> LocalLight {
    let light = LightComponent {
        light_type: LightType::Spotlight,
        intensity: 1.0,
        range,
        inner_cone: outer * 0.5,
        outer_cone: outer,
        ..crate::scene::authoring::default_light()
    };
    LocalLight::new(at, dir, &light).expect("a local light")
}

/// A float in `[lo, hi)` from `rng`.
fn range(rng: &mut Random, lo: f32, hi: f32) -> f32 {
    rng.range_f64(lo.into(), hi.into()) as f32
}

/// Check one spot in `cam`'s view: subset of its sphere, and every sampled point
/// of its range-capped cone inside the view finds it in its cluster.
fn check(cam: &Camera, l: &LocalLight, rng: &mut Random) {
    let aspect = 16.0 / 9.0;
    let grid = ClusterGrid::new(cam, aspect);
    let view_proj = cam.build_view_projection(aspect);
    let frustum = crate::render::Frustum::from_view_proj(view_proj);
    let mut cache = AabbCache::default();
    let coned = bin(&grid, std::slice::from_ref(l), &mut cache);
    let sphere = bin_spheres(&grid, &[l.sphere()], &Budget::nearest(1), &mut cache);
    for cluster in 0..CLUSTER_COUNT as u32 {
        if !lists_of(&coned, cluster).is_empty() {
            assert!(
                !lists_of(&sphere, cluster).is_empty(),
                "{l:?}: {cluster} not in its sphere"
            );
        }
    }
    let (apex, axis) = (Vec3::from(l.position), Vec3::from(l.direction));
    for _ in 0..400 {
        let p = apex + rng.inside_unit_sphere() * l.range * 0.999;
        let to = (p - apex).normalize_or_zero();
        if to.dot(axis) < l.outer_cone || !frustum.intersects_aabb(p, p) {
            continue;
        }
        let cluster = grid.cluster_at(view_proj, p);
        assert_eq!(lists_of(&coned, cluster), [0], "{l:?}: missing at {p}");
    }
}

#[test]
fn a_spots_clusters_are_within_its_sphere_and_cover_its_cone() {
    let mut rng = Random::new(0x1712_5eed);
    let mut ortho = Camera::new(Vec3::ZERO, -90.0, 0.0);
    ortho.projection = Projection::Orthographic { size: 8.0 };
    for cam in [Camera::new(Vec3::ZERO, -90.0, 0.0), ortho] {
        for case in 0..150 {
            // Every fourth apex sits on the camera's near plane.
            let z = if case % 4 == 0 {
                -cam.near
            } else {
                range(&mut rng, -30.0, 1.0)
            };
            let at = Vec3::new(range(&mut rng, -8.0, 8.0), range(&mut rng, -4.0, 4.0), z);
            let outer = range(&mut rng, 1.0, 89.0);
            let dir = rng.on_unit_sphere();
            let l = spot(at, dir, outer, range(&mut rng, 0.5, 30.0));
            check(&cam, &l, &mut rng);
        }
    }
}

#[test]
fn a_narrow_beam_lists_in_far_fewer_clusters_than_its_sphere() {
    let cam = Camera::new(Vec3::ZERO, -90.0, 0.0);
    let grid = ClusterGrid::new(&cam, 16.0 / 9.0);
    let mut cache = AabbCache::default();
    // A 50 m beacon sweeping across the view, as the issue's lab measured.
    let l = spot(Vec3::new(-5.0, 3.0, -20.0), Vec3::X, 12.0, 50.0);
    let coned = bin(&grid, &[l], &mut cache).indices.len();
    let sphere = bin_spheres(&grid, &[l.sphere()], &Budget::nearest(1), &mut cache);
    assert!(
        coned > 0 && coned * 4 < sphere.indices.len(),
        "{coned} vs {}",
        sphere.indices.len()
    );
}

#[test]
fn a_cone_too_wide_to_trim_keeps_its_range_sphere() {
    for outer in [WIDE_CONE, 90.0] {
        let b = spot(Vec3::ONE, Vec3::NEG_Z, outer, 10.0).bounds();
        assert!(b.cone.is_none(), "{outer}°");
    }
    for outer in [1.0, 30.0, 45.0, 60.0, super::cone::WIDEST_CULLED_CONE] {
        let l = spot(Vec3::ONE, Vec3::NEG_Z, outer, 10.0);
        let b = l.bounds();
        assert_eq!(b.sphere, l.sphere(), "the cut ranks by the range sphere");
        let tight = b.cone.expect("a coned spot").bounding_sphere();
        assert!(tight.1 < 10.0, "{outer}°: {tight:?}");
    }
}

/// Just past the widest cone the binner culls by.
const WIDE_CONE: f32 = super::cone::WIDEST_CULLED_CONE + 1.0;
