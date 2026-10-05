//! `baked_direct_sh` (#809): only `Baked` lights, the forward shader's falloff, and
//! static geometry shadows them.

use glam::Vec3;

use super::tests::{floor, quad};
use super::*;
use crate::scene::lighting::sh::Sh9;

/// A `Baked` (or `Mixed`) point light 2 units above the origin.
fn lamp(baked: bool) -> BakeLight {
    BakeLight {
        shape: LightShape::Point {
            position: Vec3::new(0.0, 2.0, 0.0),
            range: 10.0,
        },
        radiance: Vec3::splat(5.0),
        bakes_direct: baked,
    }
}

fn scene(meshes: Vec<BakeMesh>, light: BakeLight) -> BakeScene {
    BakeScene {
        meshes,
        lights: vec![light],
        sky: Vec3::ZERO,
    }
}

const PROBE: Vec3 = Vec3::new(0.0, 0.5, 0.0);

#[test]
fn a_mixed_light_adds_nothing() {
    let sh = baked_direct_sh(&scene(vec![floor()], lamp(false)), &[PROBE]);
    assert_eq!(sh, vec![Sh9::zero()]);
}

#[test]
fn a_baked_light_reads_back_as_its_lambert_irradiance() {
    let sh = baked_direct_sh(&scene(vec![floor()], lamp(true)), &[PROBE])[0];
    // 1.5 units below the lamp: 5 / (1.5² + 1), facing it head-on.
    let expected = 5.0 / (1.5 * 1.5 + 1.0);
    let toward = sh.eval(Vec3::Y).x;
    // An L2 delta overshoots a clamped cosine by ~6% head-on.
    assert!(
        (toward - expected).abs() < expected * 0.1,
        "{toward} vs {expected}"
    );
    assert!(sh.eval(Vec3::NEG_Y).x < toward * 0.1, "lit from above only");
}

#[test]
fn static_geometry_shadows_a_baked_light() {
    // A ceiling at y = 1 (facing down) between the probe and the lamp.
    let p = |x, z| Vec3::new(x, 1.0, z);
    let ceiling = quad(
        3,
        [p(-2.0, -2.0), p(2.0, -2.0), p(2.0, 2.0), p(-2.0, 2.0)],
        Vec3::ONE,
    );
    let sh = baked_direct_sh(&scene(vec![floor(), ceiling], lamp(true)), &[PROBE]);
    assert_eq!(sh, vec![Sh9::zero()]);
}

#[test]
fn a_probe_out_of_range_gets_nothing() {
    let far = Vec3::new(0.0, 2.0, 20.0);
    let sh = baked_direct_sh(&scene(vec![], lamp(true)), &[PROBE, far]);
    assert!(sh[0].eval(Vec3::Y).x > 0.0);
    assert_eq!(sh[1], Sh9::zero());
}
