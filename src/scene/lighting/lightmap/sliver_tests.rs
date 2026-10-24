//! Sliver triangles (#1330): a hit on a 100 m × 1 cm triangle still lands at finite
//! barycentrics on the point the ray crossed, so its bounce carries on from a real
//! point and a shadow next to it stays dark.

use glam::{Vec2, Vec3};

use super::rays::{Rays, Tri};
use super::run_bake_tests::halves;
use super::tests::{floor, quad, settings};
use super::*;

/// A 100 m × 1 cm triangle in the plane `y`, from `x` to `x + 0.01`, facing down.
fn sliver(x: f32, y: f32) -> [Vec3; 3] {
    [
        Vec3::new(x, y, -50.0),
        Vec3::new(x + 0.01, y, 0.0),
        Vec3::new(x, y, 50.0),
    ]
}

#[test]
fn a_sliver_hit_has_finite_barycentrics_at_the_hit_point() {
    let [a, b, c] = sliver(0.0, 0.0);
    let rays = Rays::build(vec![Tri::new(a, b, c, 0, 0)]);
    let hit = rays
        .closest(Vec3::new(0.004, 1.0, 0.0), Vec3::NEG_Y, 1e-4, f32::MAX)
        .expect("the sliver is in the way");
    let (u, v) = (hit.u, hit.v);
    assert!(
        (0.0..=1.0).contains(&u) && (0.0..=1.0).contains(&v),
        "{hit:?}"
    );
    assert!((u - 0.4).abs() < 1e-4 && (v - 0.3).abs() < 1e-4, "{hit:?}");
    let t = &rays.tris[0];
    let at = t.a + t.e1 * u + t.e2 * v;
    assert!(at.distance(Vec3::new(0.004, 0.0, 0.0)) < 1e-3, "{at}");
}

/// The floor's mean light under a roof over x < 0 at y = 1, then in the open, with
/// or without a grating of slivers between floor and roof, under an open sky. A
/// ground under it all catches what bounces down off the slivers' far ends.
fn floor_halves(grating: bool) -> (f32, f32) {
    let at = |y: f32| move |x, z| Vec3::new(x, y, z);
    let (p, g) = (at(1.0), at(-0.01));
    let roof = quad(
        2,
        [p(-20.0, 60.0), p(-20.0, -60.0), p(0.0, -60.0), p(0.0, 60.0)],
        Vec3::splat(0.5),
    );
    let ground = quad(
        4,
        [
            g(-60.0, 60.0),
            g(60.0, 60.0),
            g(60.0, -60.0),
            g(-60.0, -60.0),
        ],
        Vec3::splat(0.5),
    );
    let positions: Vec<Vec3> = (0..200)
        .flat_map(|k| sliver(-2.0 + k as f32 * 0.01, 0.5))
        .collect();
    let count = positions.len();
    let slivers = BakeMesh {
        entity: 3,
        normals: vec![Vec3::NEG_Y; count],
        lightmap_uvs: vec![Vec2::ZERO; count],
        indices: (0..count as u32).collect(),
        positions,
        runs: vec![BakeRun {
            end: count as u32,
            ..roof.runs[0]
        }],
    };
    let mut meshes = vec![floor(), roof, ground];
    if grating {
        meshes.push(slivers);
    }
    let scene = BakeScene {
        meshes,
        lights: Vec::new(),
        sky: Vec3::ONE,
    };
    let maps = bake(&scene, &settings());
    let map = maps.iter().find(|m| m.entity == 1).expect("floor baked");
    assert!(map.texels.iter().all(|t| t.is_finite()), "a NaN texel");
    halves(map)
}

#[test]
fn slivers_under_a_roof_keep_its_shadow() {
    // The floor's rays hit the slivers' fronts and bounce on from where they landed:
    // the shaded half stays dark, and no darker-than-roof grating lights it up.
    let (bare, _) = floor_halves(false);
    let (shaded, open) = floor_halves(true);
    assert!(shaded < 0.5 * open, "{shaded} | {open}, bare {bare}");
    assert!(
        shaded <= bare + 0.02,
        "{shaded} under slivers, {bare} without"
    );
}
