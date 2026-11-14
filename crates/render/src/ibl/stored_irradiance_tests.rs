//! The irradiance a reflection bake stores beside each cube (#1697): its capture's
//! SH, which reads back the room's colours, with the scene's ambient where the view
//! escapes, as a light probe at that point holds it, whatever the ambient source.

use glam::Vec3;

use std::f32::consts::PI;

use super::reflection_bake_tests::colored_room;
use crate::ibl::reflection_probes::uniform::{luminance_sh, normalisation};
use crate::Renderer;
use rusty_core::color::luminance;
use rusty_scene::authoring::{primitive_mesh_component, Primitive};
use rusty_scene::lighting::sh::Sh9;
use rusty_scene::{AmbientMode, Scene};
use rusty_test_support::panorama::fixture as panorama_fixture;

const RES: u32 = 32;

/// `scene` with one probe at the origin, baked; its stored irradiance along `n`.
fn bake(mut scene: Scene, name: &str, n: Vec3) -> Option<Vec3> {
    let mut renderer = crate::test_gpu::headless_or_skip(RES, RES)?;
    scene.lighting_settings.reflections.resolution = RES;
    scene.reflection_probes.add(Vec3::ZERO, Vec3::splat(5.0));
    let dir =
        rusty_test_support::temp::dir().join(format!("rusty_1697_{name}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    renderer.bake_reflections(&mut scene, &dir).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    let stored = scene.reflection_probes.probes[0].irradiance;
    Some(stored.expect("the bake stores the irradiance").eval(n))
}

#[test]
fn gpu_a_bake_stores_the_irradiance_its_cube_gives() {
    // The +X wall glows red, the -X wall green.
    let Some(red) = bake(colored_room(), "room_x", Vec3::X) else {
        return;
    };
    let green = bake(colored_room(), "room_negx", Vec3::NEG_X).unwrap();
    assert!(red.x > red.y && red.x > 1.0, "facing the red wall: {red}");
    assert!(
        green.y > green.x && green.y > 1.0,
        "facing the green wall: {green}"
    );
}

/// A probe 1 m over an open, unoccluded floor stores what a light probe there holds
/// (the sky the capture draws is not what the lightmap sees), so the floor keeps its
/// reflection, for every ambient source. With no ambient the sky the capture draws
/// is not stored either: only the floor's own sheen of it is, which the lightmap
/// lacks (the PR's note).
#[test]
fn gpu_an_open_sky_probe_stores_what_a_light_probe_holds() {
    let Some(mut renderer) = crate::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    let white = panorama_fixture("1697_white.png", [255; 3], [255; 3], 255);
    let cases: [(&str, AmbientMode, f32, &str); 5] = [
        ("color", AmbientMode::Color, 1.0, ""),
        ("gradient", AmbientMode::Gradient, 1.0, ""),
        ("procedural", AmbientMode::Skybox, 1.0, ""),
        ("panorama_5", AmbientMode::Skybox, 5.0, &white),
        ("no_ambient", AmbientMode::Color, 0.0, ""),
    ];
    for (name, mode, intensity, sky) in cases {
        let mut scene = Scene::new();
        (scene.ambient.mode, scene.ambient_intensity) = (mode, intensity);
        scene.skybox_path = sky.to_string();
        let floor = scene.add_entity("Floor".to_string());
        let mesh = primitive_mesh_component(Primitive::Plane);
        scene.world.set_mesh(floor, mesh);
        scene.world.set_static(floor, true);
        scene.world.transform_mut(floor).unwrap().scale = Vec3::splat(10.0);
        let at = Vec3::Y;
        let light_probe = luminance(renderer.bake_probe(&scene, at, RES).eval(Vec3::Y)) / PI;
        let stored = stored_sh(&mut renderer, scene, at, name);
        let ratio = normalisation(luminance_sh(stored), Vec3::Y, light_probe);
        let lum = luminance(stored.eval(Vec3::Y)) / PI;
        eprintln!("[1697] {name}: stored {lum:.4}, light probe {light_probe:.4}, scale {ratio:.3}");
        if intensity == 0.0 {
            assert!(lum < 0.01, "the drawn sky is not stored: {lum}");
            continue;
        }
        assert!(
            ratio > 0.9,
            "{name}: the floor keeps its reflection, {ratio}"
        );
        assert!(
            (lum - light_probe).abs() <= 0.15 * light_probe + 2e-3,
            "{name}: stored {lum} vs light probe {light_probe}"
        );
    }
}

/// `scene` with one probe at `at`, baked by `renderer`; its stored irradiance.
fn stored_sh(renderer: &mut Renderer, mut scene: Scene, at: Vec3, name: &str) -> Sh9 {
    scene.lighting_settings.reflections.resolution = RES;
    scene.reflection_probes.add(at, Vec3::splat(5.0));
    let dir = rusty_test_support::temp::dir().join(format!("rusty_1697_open_{name}"));
    std::fs::create_dir_all(&dir).unwrap();
    renderer.bake_reflections(&mut scene, &dir).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    scene.reflection_probes.probes[0]
        .irradiance
        .expect("stored")
}
