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
fn a_floor_under_area_panels_spends_most_of_its_rays_on_its_own_direct_light() {
    // The measurement #2355 is conditioned on: the share of the floor's rays that
    // are its own direct light's shadow rays, at the default settings.
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
    assert!(share > 1.0 / 3.0, "direct rays are {share:.2} of {cost:?}");
}
