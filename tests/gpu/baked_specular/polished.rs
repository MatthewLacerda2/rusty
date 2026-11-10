//! A polished surface under a `Baked` spot (#1996): the issue's black granite
//! counter (roughness 0.06) a metre under a spot, baked at 8 texels a unit. Its
//! baked highlight, seen close enough that one lightmap texel covers most of the
//! frame, is the live lamp's: round and smooth, where a direction page off by a
//! few degrees a texel (BC7's) drew it as a sawtooth blot of texel-sized steps.

use std::f32::consts::FRAC_PI_2;

use glam::{Quat, Vec3};
use rusty::components::{LightComponent, LightMode, LightType, MaterialAsset};
use rusty::scene::{Camera, Scene};

use super::{bake, lamp, mean_gap, most, plane, point, shot};

/// A 3 m static counter of polished black granite a metre under a spot of `mode`.
fn counter(mode: LightMode) -> Scene {
    let mut scene = Scene::new();
    crate::gpu::black_sky(&mut scene);
    scene.ambient_intensity = 0.0;
    let granite = MaterialAsset {
        base_color: [0.03; 3],
        roughness: 0.06,
        ..MaterialAsset::default()
    };
    plane(
        &mut scene,
        "counter",
        Vec3::ZERO,
        Quat::IDENTITY,
        0.2,
        granite,
    );
    let spot = LightComponent {
        light_type: LightType::Spotlight,
        intensity: 6.0,
        range: 4.2,
        outer_cone: 90.0,
        inner_cone: 70.0,
        ..point(mode)
    };
    lamp(&mut scene, Vec3::Y, Quat::from_rotation_x(-FRAC_PI_2), spot);
    scene
}

/// `scene` baked at the issue's 8 texels a unit (`directional` or not), then shot
/// through a long lens at the spot's mirror image, 0.53 m off its foot.
fn baked(mut scene: Scene, name: &str, directional: bool) -> Option<Vec<f32>> {
    bake(&mut scene, name, directional, 8.0);
    let mut camera = Camera::new(Vec3::new(-1.0, 0.9, 0.0), 0.0, -62.2);
    camera.fov = 8.0;
    shot(&scene, &camera, name)
}

#[test]
fn a_polished_counters_baked_highlight_is_the_live_one_texel_for_texel() {
    let Some(lobed) = baked(counter(LightMode::Baked), "polished_lobed", true) else {
        eprintln!("[baked-spec] no GPU/software adapter — skipping");
        return;
    };
    let live = baked(counter(LightMode::Mixed), "polished_live", false).unwrap();
    let peak = live.iter().copied().fold(0.0, f32::max);
    let mean = mean_gap(&lobed, &live);
    let worst = most(&lobed, &live);
    eprintln!("[baked-spec] polished: mean {mean:.2}, at most {worst:.1} apart (peak {peak:.0})");
    assert!(peak > 200.0, "the live spot shows a highlight: {peak:.0}");
    assert!(
        mean <= 4.0,
        "the baked highlight is the live one: {mean:.2}"
    );
    assert!(worst <= 48.0, "no step or dot stands out of it: {worst:.1}");
}
