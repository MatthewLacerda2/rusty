//! `Skybox` mode's light (#1675): the projection, its memo, and what the scene draws.

use glam::Vec3;

use super::{panorama, panorama_fixture, procedural};
use crate::scene::lighting::probe_fill::fibonacci_sphere;
use crate::scene::{AmbientMode, AmbientRadiance, LightType, Scene, SkySettings};

const NORMALS: [Vec3; 5] = [
    Vec3::Y,
    Vec3::NEG_Y,
    Vec3::X,
    Vec3::NEG_X,
    Vec3::new(0.0, 0.6, 0.8),
];

fn skybox_scene(path: &str) -> Scene {
    let mut scene = Scene::new();
    scene.ambient.mode = AmbientMode::Skybox;
    scene.skybox_path = path.to_string();
    scene
}

/// The bands converge as the rows grow (16 rows ring 1% into L2 at the poles).
#[test]
fn a_constant_panorama_lights_every_normal_alike() {
    let image = image::RgbaImage::from_pixel(128, 64, image::Rgba([51, 102, 204, 255]));
    let sky = AmbientRadiance::Sky(panorama::project(128, 64, image.as_raw()));
    let want = Vec3::new(0.2, 0.4, 0.8);
    for n in NORMALS {
        assert!(sky.at(n).abs_diff_eq(want, 2e-3), "{n}: {}", sky.at(n));
        assert!(
            sky.incoming(n).abs_diff_eq(want, 2e-3),
            "{n}: {}",
            sky.incoming(n)
        );
    }
}

/// The sky's SH is a radiance, so its cosine average is `at` exactly (no L2 cut on
/// top of the projection's own), as the trace's and the probes' tests rely on.
#[test]
fn the_skys_incoming_cosine_averages_to_its_unbaked_ambient() {
    let dirs = fibonacci_sphere(20_000);
    let path = panorama_fixture("avg.png", [60, 100, 220], [160, 110, 60], 40);
    let skies = [
        Scene::new().sky,
        SkySettings {
            cloud_coverage: 1.0,
            ..SkySettings::default()
        },
    ];
    let mut cases: Vec<_> = skies
        .iter()
        .map(|s| AmbientRadiance::Sky(procedural::radiance(s, Vec3::new(0.3, 0.5, 0.9))))
        .collect();
    cases.push(skybox_scene(&path).drawn_ambient().radiance());
    for sky in cases {
        for n in NORMALS {
            let sum: Vec3 = dirs
                .iter()
                .map(|d| sky.incoming(*d) * n.dot(*d).max(0.0))
                .sum();
            let average = sum * (4.0 / dirs.len() as f32);
            assert!(
                average.abs_diff_eq(sky.at(n), 2e-3),
                "{n}: {average} vs {}",
                sky.at(n)
            );
            assert!(sky.at(n).abs_diff_eq(sky.at_l2(n).max(Vec3::ZERO), 1e-5));
        }
    }
}

#[test]
fn the_panorama_lights_by_direction_and_follows_its_file() {
    let path = panorama_fixture("dir.png", [60, 100, 220], [160, 110, 60], 40);
    let mut scene = skybox_scene(&path);
    scene.ambient_intensity = 2.0;
    let sky = scene.drawn_ambient().radiance();
    let (up, down) = (sky.at(Vec3::Y), sky.at(Vec3::NEG_Y));
    assert!(
        up.z > down.z && down.x > up.x,
        "blue above, orange below: {up} {down}"
    );
    assert!(sky.at(Vec3::X).x > sky.at(Vec3::NEG_X).x, "the warmer east");
    // Intensity scales it; another file, and the same file rewritten, re-project.
    scene.ambient_intensity = 1.0;
    assert!(scene
        .drawn_ambient()
        .radiance()
        .at(Vec3::Y)
        .abs_diff_eq(up * 0.5, 1e-6));
    let other = panorama_fixture("other.png", [160, 110, 60], [60, 100, 220], 0);
    scene.skybox_path = other;
    let flipped = scene.drawn_ambient().radiance().at(Vec3::Y);
    assert!(flipped.x > flipped.z, "orange above now: {flipped}");
    panorama_fixture("dir.png", [10, 10, 10], [10, 10, 10], 0);
    scene.skybox_path = path;
    let dim = scene.drawn_ambient().radiance().at(Vec3::Y);
    assert!(dim.abs_diff_eq(Vec3::splat(10.0 / 255.0), 2e-3), "{dim}");
}

#[test]
fn the_procedural_sky_lights_without_a_panorama_and_is_affine_in_its_colour() {
    let scene = skybox_scene("");
    let drawn = scene.drawn_ambient();
    assert_eq!(drawn.gradient, None);
    let sky = drawn.radiance();
    assert!(sky.at(Vec3::Y).max_element() > sky.at(Vec3::NEG_Y).max_element());
    // The memo combines two fixed terms; projecting afresh under the colour agrees.
    let color = Vec3::new(0.9, 0.2, 0.4);
    let s = SkySettings::default();
    let (memo, fresh) = (
        procedural::radiance(&s, color),
        procedural::project(&s, color),
    );
    for n in NORMALS {
        assert!(memo.eval(n).abs_diff_eq(fresh.eval(n), 1e-4), "{n}");
    }
    // A panorama that cannot be read falls back to the procedural sky.
    let missing = skybox_scene("no/such/sky.png").drawn_ambient();
    assert_eq!(missing.skybox, drawn.skybox);
}

#[test]
fn an_ambient_light_overrides_the_sky_and_the_mode_saves() {
    let mut scene = skybox_scene("");
    let id = scene.add_entity("Ambient".into());
    let light = crate::components::LightComponent {
        light_type: LightType::Ambient,
        color: Vec3::X,
        ..crate::scene::authoring::default_light()
    };
    scene.world.set_light(id, Some(light));
    let drawn = scene.drawn_ambient();
    assert_eq!((drawn.skybox, drawn.color), (None, Vec3::X));
    let json = serde_json::to_string(&crate::scene::to_scene_data(&scene)).unwrap();
    assert!(
        json.contains("\"Skybox\"") && !json.contains("sky_sh"),
        "{json}"
    );
}
