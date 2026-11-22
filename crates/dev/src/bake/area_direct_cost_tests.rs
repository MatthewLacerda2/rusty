use glam::Vec3;

use rusty_lighting::lightmap::{trace_texels, AreaShape, BakeLight, BakeScene, LightShape};
use rusty_scene::lighting::settings::BakeSettings;
use rusty_scene::test_levels::block;
use rusty_scene::Scene;

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
    let direct = |direct_samples| {
        let settings = BakeSettings {
            direct_samples,
            ..Default::default()
        };
        trace_texels(&panels_over_floor(), &settings, 200, 1).direct
    };
    // 16 samples are too few for the probe, so all 16 rays; at 64 the probe's 20 rays and its
    // checks stay under twice that.
    let (sixteen, probed) = (direct(16), direct(64));
    println!("16 direct samples: {sixteen} rays; 64, probed: {probed}");
    assert!(
        probed < sixteen * 2,
        "{probed} against {sixteen} (exhaustive: 4×)"
    );
}
