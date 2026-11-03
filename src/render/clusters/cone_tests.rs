//! Cone-culled spot binning (#1712): over random spotlights, near the camera and
//! straddling its near plane, a spot's clusters are a subset of its range
//! sphere's and include every cluster its cone reaches, in a perspective and an
//! orthographic view; a cone too wide to trim keeps its range sphere.

use glam::Vec3;

use super::bin::{bin_spheres, Binned, Budget};
use super::grid::AabbCache;
use super::*;
use crate::components::{LightComponent, Projection};
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

/// A small deterministic generator: xorshift, `[0, 1)`.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 40) as f32 / (1u64 << 24) as f32
    }
    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.next()
    }
    fn in_ball(&mut self) -> Vec3 {
        loop {
            let v = Vec3::new(
                self.range(-1.0, 1.0),
                self.range(-1.0, 1.0),
                self.range(-1.0, 1.0),
            );
            if v.length_squared() <= 1.0 && v.length_squared() > 1e-6 {
                return v;
            }
        }
    }
}

fn list(b: &Binned, cluster: usize) -> &[u32] {
    let [offset, count] = b.ranges[cluster];
    &b.indices[offset as usize..(offset + count) as usize]
}

/// Check one spot in `cam`'s view: subset of its sphere, and every sampled point
/// of its range-capped cone inside the view finds it in its cluster.
fn check(cam: &Camera, l: &LocalLight, rng: &mut Rng) {
    let aspect = 16.0 / 9.0;
    let grid = ClusterGrid::new(cam, aspect);
    let view_proj = cam.build_view_projection(aspect);
    let frustum = crate::render::Frustum::from_view_proj(view_proj);
    let mut cache = AabbCache::default();
    let coned = bin(&grid, std::slice::from_ref(l), &mut cache);
    let sphere = bin_spheres(&grid, &[l.sphere()], &Budget::nearest(1), &mut cache);
    for cluster in 0..CLUSTER_COUNT {
        if !list(&coned, cluster).is_empty() {
            assert!(
                !list(&sphere, cluster).is_empty(),
                "{l:?}: {cluster} not in its sphere"
            );
        }
    }
    let (apex, axis) = (Vec3::from(l.position), Vec3::from(l.direction));
    for _ in 0..400 {
        let p = apex + rng.in_ball() * l.range * 0.999;
        let to = p - apex;
        if to.normalize().dot(axis) < l.outer_cone || !frustum.intersects_aabb(p, p) {
            continue;
        }
        let cluster = grid.cluster_at(view_proj, p) as usize;
        assert_eq!(list(&coned, cluster), [0], "{l:?}: missing at {p}");
    }
}

#[test]
fn a_spots_clusters_are_within_its_sphere_and_cover_its_cone() {
    let mut rng = Rng(0x1712_5eed);
    let mut ortho = Camera::new(Vec3::ZERO, -90.0, 0.0);
    ortho.projection = Projection::Orthographic { size: 8.0 };
    for cam in [Camera::new(Vec3::ZERO, -90.0, 0.0), ortho] {
        for case in 0..150 {
            // Every fourth apex sits on the camera's near plane.
            let z = if case % 4 == 0 {
                -cam.near
            } else {
                rng.range(-30.0, 1.0)
            };
            let at = Vec3::new(rng.range(-8.0, 8.0), rng.range(-4.0, 4.0), z);
            let outer = rng.range(1.0, 89.0);
            let l = spot(at, rng.in_ball(), outer, rng.range(0.5, 30.0));
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
        let l = spot(Vec3::ONE, Vec3::NEG_Z, outer, 10.0);
        let b = l.bounds();
        assert!(b.cone.is_none(), "{outer}°");
        assert_eq!(b.sphere, l.sphere());
    }
    let b = spot(Vec3::ONE, Vec3::NEG_Z, 30.0, 10.0).bounds();
    assert!(b.cone.is_some() && b.sphere.1 < 10.0, "{b:?}");
}

/// Just past the widest cone the binner culls by.
const WIDE_CONE: f32 = super::cone::WIDEST_CULLED_CONE + 1.0;
