//! Area lights in the bake (#1674): intensity × area far away, one-sided, cut at
//! `range`, and penumbrae that widen with the panel.

use glam::Vec3;

use super::bake::triangles;
use super::lights::area::AreaShape;
use super::rays::Rays;
use super::rng::Rng;
use super::tests::quad;
use super::trace::Tracer;
use super::*;

const RADIANCE: f32 = 4.0;

/// A `w × h` panel at height `y` over the origin, facing down.
fn panel(w: f32, h: f32, y: f32, range: f32) -> BakeLight {
    BakeLight {
        shape: LightShape::Area(AreaShape {
            center: Vec3::new(0.0, y, 0.0),
            right: Vec3::X * w,
            up: Vec3::Z * h,
            forward: Vec3::NEG_Y,
            range,
        }),
        radiance: Vec3::splat(RADIANCE),
        bakes_direct: true,
    }
}

/// The baked direct irradiance `E` at `p` facing `normal`.
fn direct_at(scene: &BakeScene, p: Vec3, normal: Vec3) -> f32 {
    let rays = Rays::build(triangles(scene));
    let tracer = Tracer { scene, rays: &rays };
    let mut rng = Rng::new(3, 0, 0);
    tracer.texel(p, normal, 1024, 1, &mut rng).direct.x * std::f32::consts::PI
}

fn lit(lights: Vec<BakeLight>, meshes: Vec<BakeMesh>) -> BakeScene {
    BakeScene {
        meshes,
        lights,
        sky: Vec3::ZERO.into(),
    }
}

#[test]
fn far_away_a_panel_is_a_point_light_of_intensity_times_area() {
    let d: f32 = 20.0;
    for (w, h) in [(1.2, 0.6), (2.0, 0.5), (0.3, 0.3)] {
        let scene = lit(vec![panel(w, h, d, 100.0)], vec![]);
        let e = direct_at(&scene, Vec3::ZERO, Vec3::Y);
        let point = RADIANCE * w * h / (d * d + 1.0);
        assert!((e - point).abs() < point * 0.01, "{w}×{h}: {e} vs {point}");
    }
    // 60° off the panel's facing, the receiver turned toward it: cos 60° as much.
    let scene = lit(vec![panel(1.0, 1.0, 10.0, 100.0)], vec![]);
    let at = Vec3::new(10.0 * 3f32.sqrt(), 0.0, 0.0);
    let toward = (Vec3::new(0.0, 10.0, 0.0) - at).normalize();
    let e = direct_at(&scene, at, toward);
    let expected = RADIANCE * 0.5 / (400.0 + 1.0);
    assert!((e - expected).abs() < expected * 0.02, "{e} vs {expected}");
}

#[test]
fn a_panel_is_one_sided_and_cut_at_its_range() {
    let scene = lit(vec![panel(1.0, 1.0, 2.0, 5.0)], vec![]);
    assert!(direct_at(&scene, Vec3::ZERO, Vec3::Y) > 0.0);
    let above = Vec3::new(0.0, 4.0, 0.0);
    assert_eq!(direct_at(&scene, above, Vec3::NEG_Y), 0.0, "behind it");
    let far = Vec3::new(6.0, 1.9, 0.0);
    assert_eq!(direct_at(&scene, far, Vec3::NEG_X), 0.0, "past range");
}

/// The width of the shadow edge a blocker covering `x < 0` halfway up casts on
/// the floor: where the light is between 10% and 90% of the open floor's.
fn penumbra(light: BakeLight) -> f32 {
    let p = |x, z| Vec3::new(x, 1.5, z);
    let blocker = quad(
        9,
        [p(-4.0, 4.0), p(0.0, 4.0), p(0.0, -4.0), p(-4.0, -4.0)],
        Vec3::ONE,
    );
    let (shadowed, open) = (lit(vec![light], vec![blocker]), lit(vec![light], vec![]));
    let step = 0.05;
    let partial = (-40..=40)
        .map(|k| Vec3::new(k as f32 * step, 0.0, 0.0))
        .filter(|&at| {
            let f = direct_at(&shadowed, at, Vec3::Y) / direct_at(&open, at, Vec3::Y);
            (0.1..0.9).contains(&f)
        })
        .count();
    partial as f32 * step
}

#[test]
fn the_penumbra_widens_with_the_panel() {
    let point = BakeLight {
        shape: LightShape::Point {
            position: Vec3::new(0.0, 3.0, 0.0),
            range: 10.0,
        },
        radiance: Vec3::splat(RADIANCE),
        bakes_direct: true,
    };
    let (hard, small, wide) = (
        penumbra(point),
        penumbra(panel(0.4, 0.6, 3.0, 10.0)),
        penumbra(panel(1.2, 0.6, 3.0, 10.0)),
    );
    assert!(hard <= 0.05, "a point light's edge is hard: {hard}");
    // Halfway between panel and floor the penumbra is as wide as the panel, and
    // its 10–90% span about 80% of that.
    assert!((small - 0.8 * 0.4).abs() < 0.1, "0.4 m panel: {small}");
    assert!((wide - 0.8 * 1.2).abs() < 0.15, "1.2 m panel: {wide}");
}

#[test]
fn a_probe_under_a_panel_gets_its_direct_light_from_the_front_only() {
    let scene = lit(vec![panel(1.2, 0.6, 20.0, 100.0)], vec![]);
    let probes = [Vec3::ZERO, Vec3::new(0.0, 25.0, 0.0)];
    let sh = baked_direct_sh(&scene, &probes);
    let point = RADIANCE * 0.72 / 401.0;
    let toward = sh[0].eval(Vec3::Y).x;
    // An L2 delta overshoots a clamped cosine by ~6% head-on.
    assert!((toward - point).abs() < point * 0.1, "{toward} vs {point}");
    assert_eq!(sh[1], crate::scene::lighting::sh::Sh9::zero(), "behind it");
}
