//! Area lights at their edges (#1674): a panel mounted flush on a ceiling lights
//! the floor evenly instead of shadowing itself, and a panel that can't reach a
//! texel takes nothing from its seeded stream, so it changes none of its bytes.

use glam::Vec3;

use super::area_tests::{direct_at, lit, panel};
use super::bake::triangles;
use super::rays::Rays;
use super::rng::Rng;
use super::tests::quad;
use super::trace::Tracer;
use super::*;

/// An 8 × 8 static ceiling at height `y`, facing down.
fn ceiling(y: f32) -> BakeMesh {
    let p = |x, z| Vec3::new(x, y, z);
    quad(
        7,
        [p(-4.0, -4.0), p(4.0, -4.0), p(4.0, 4.0), p(-4.0, 4.0)],
        Vec3::splat(0.5),
    )
}

#[test]
fn a_panel_flush_on_a_ceiling_lights_an_even_pool() {
    let light = panel(1.2, 0.6, 3.0, 20.0);
    let (flush, open) = (
        lit(vec![light], vec![ceiling(3.0)]),
        lit(vec![light], vec![]),
    );
    for k in -8..=8 {
        for at in [
            Vec3::new(k as f32 * 0.25, 0.0, 0.0),
            Vec3::new(0.0, 0.0, k as f32 * 0.25),
        ] {
            let (e, expected) = (
                direct_at(&flush, at, Vec3::Y),
                direct_at(&open, at, Vec3::Y),
            );
            assert!(expected > 0.0, "{at} is lit in the open");
            // Within the jitter: bounces off the ceiling draw from the same stream.
            assert!(
                (e - expected).abs() <= expected * 1e-2,
                "{at}: {e} vs {expected}"
            );
        }
    }
    let probes: Vec<Vec3> = (-4..=4)
        .map(|k| Vec3::new(k as f32 * 0.5, 1.0, 0.3))
        .collect();
    let (shut, free) = (
        baked_direct_sh(&flush, &probes),
        baked_direct_sh(&open, &probes),
    );
    for (s, f) in shut.iter().zip(&free) {
        let (s, f) = (s.eval(Vec3::Y).x, f.eval(Vec3::Y).x);
        assert!(f > 0.0 && (s - f).abs() <= f * 1e-4, "probe: {s} vs {f}");
    }
}

/// A texel with bounces under a ceiling, and the light it bakes.
fn texel(lights: Vec<BakeLight>) -> trace::TexelLight {
    let mut scene = lit(lights, vec![ceiling(1.0)]);
    scene.sky = Vec3::ONE.into();
    let rays = Rays::build(triangles(&scene));
    let tracer = Tracer {
        scene: &scene,
        rays: &rays,
    };
    tracer.texel(Vec3::ZERO, Vec3::Y, 64, 3, &mut Rng::new(5, 1, 2))
}

#[test]
fn a_panel_out_of_reach_changes_no_bytes() {
    let alone = texel(vec![]);
    // Past its range, and in range but behind it (facing down, below the texel
    // and every bounce off the ceiling).
    for far in [panel(1.0, 1.0, -50.0, 5.0), panel(1.0, 1.0, -0.5, 50.0)] {
        let with = texel(vec![far]);
        assert_eq!(with.indirect.to_array(), alone.indirect.to_array());
        assert_eq!(with.indirect_dir.to_array(), alone.indirect_dir.to_array());
        assert_eq!(with.direct, Vec3::ZERO);
    }
}
