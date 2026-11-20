//! What a texel's own direct light from Area lights costs (#2355), in rays: a
//! floor under a 4 × 4 grid of ceiling panels, a box on it casting a penumbra,
//! the shape of a bar's floor under its Area panels. Counts, never timings, so it
//! holds on any machine (`docs/testing.md`).

use glam::Vec3;

use rusty_lighting::lightmap::{trace_texels, AreaShape, BakeLight, BakeScene, LightShape};
use rusty_scene::lighting::settings::BakeSettings;
use rusty_scene::test_levels::block;
use rusty_scene::Scene;

/// A 12 m floor, a 1 m box in its middle, and sixteen 1 × 0.5 m panels 3 m up
/// facing down, every one in reach of the whole floor.
pub(in crate::bake) fn panels_over_floor() -> BakeScene {
    let mut level = Scene::new();
    level.skybox_path = String::new();
    block(
        &mut level,
        "Floor",
        Vec3::new(0.0, -0.2, 0.0),
        Vec3::new(12.0, 0.0, 12.0),
    );
    block(
        &mut level,
        "Box",
        Vec3::new(5.5, 0.0, 5.5),
        Vec3::new(6.5, 1.0, 6.5),
    );
    let mut scene = BakeScene::gather(&level, &|_, _| None);
    for (i, j) in (0..4).flat_map(|i| (0..4).map(move |j| (i, j))) {
        let center = Vec3::new(1.5 + 3.0 * i as f32, 3.0, 1.5 + 3.0 * j as f32);
        scene.lights.push(BakeLight {
            shape: LightShape::Area(AreaShape {
                center,
                right: Vec3::X * 0.5,
                up: Vec3::Z * 0.25,
                forward: Vec3::NEG_Y,
                range: 12.0,
            }),
            radiance: Vec3::splat(2.0),
            bakes_direct: true,
            mask_channel: None,
        });
    }
    scene
}

#[test]
fn the_cost_bench_counts_a_floors_own_direct_rays_apart() {
    // The share of the floor's rays that are its own direct light's shadow rays,
    // at the default settings, measured: 67 520 of 90 212 (75%) before the probe
    // (#2355), 21 500 of 44 192 (49%) after.
    let settings = BakeSettings::default();
    let cost = trace_texels(&panels_over_floor(), &settings, 200, 1);
    let share = cost.direct as f64 / cost.rays as f64;
    println!(
        "{} texels, {} rays, {} of them direct ({:.0}%)",
        cost.texels,
        cost.rays,
        cost.direct,
        share * 100.0
    );
    assert!(cost.texels > 0 && cost.direct > 0, "{cost:?}");
    assert!(cost.direct < cost.rays, "{cost:?}");
}

#[test]
fn the_shadow_probe_casts_a_fraction_of_the_full_sets_rays() {
    // At 16 direct samples a panel casts 16 rays, its whole set and no probe; an
    // exhaustive bake at 64 would cast four times that. The probe casts 20 where
    // a panel is fully seen (its corners and a point in each of 4 × 4 blocks), 52
    // where it is hidden (two more rounds confirm it), and 4 + 64 in a penumbra,
    // the probe's points counted among the 64.
    let direct = |direct_samples| {
        let settings = BakeSettings {
            direct_samples,
            ..Default::default()
        };
        trace_texels(&panels_over_floor(), &settings, 200, 1).direct
    };
    let (sixteen, probed) = (direct(16), direct(64));
    println!("16 direct samples: {sixteen} rays; 64, probed: {probed}");
    assert!(
        probed < sixteen * 2,
        "{probed} against {sixteen} (exhaustive: 4×)"
    );
}
