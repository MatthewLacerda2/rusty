//! The ambient source (#1647): saving, the drawn rule, and the radiance function.

use glam::Vec3;

use super::*;
use crate::scene::{apply_scene_data, to_scene_data, SceneData};

fn round_trip(scene: &Scene) -> Scene {
    let json = serde_json::to_string(&to_scene_data(scene)).unwrap();
    let mut back = Scene::new();
    apply_scene_data(&mut back, serde_json::from_str::<SceneData>(&json).unwrap());
    back
}

#[test]
fn the_default_stays_out_of_the_file_and_a_gradient_saves() {
    let scene = Scene::new();
    let json = serde_json::to_string(&to_scene_data(&scene)).unwrap();
    assert!(!json.contains("\"ambient\""), "{json}");
    let mut scene = Scene::new();
    scene.ambient.mode = AmbientMode::Gradient;
    scene.ambient.ground = Vec3::new(0.4, 0.2, 0.1);
    assert_eq!(round_trip(&scene).ambient, scene.ambient);
    let partial: AmbientSettings = serde_json::from_str(r#"{"mode":"Gradient"}"#).unwrap();
    assert_eq!(partial.sky, AmbientSettings::default().sky);
}

#[test]
fn mode_names_round_trip() {
    for mode in AmbientMode::ALL {
        assert_eq!(AmbientMode::from_name(mode.name()), Some(mode));
    }
    assert_eq!(
        AmbientMode::from_name("gradient"),
        Some(AmbientMode::Gradient)
    );
    assert_eq!(AmbientMode::from_name("Skybox"), None);
}

#[test]
fn the_drawn_ambient_is_the_scenes_source_unless_an_ambient_light_overrides() {
    let mut scene = Scene::new();
    assert_eq!(scene.drawn_ambient().gradient, None);
    scene.ambient.mode = AmbientMode::Gradient;
    let a = scene.ambient;
    assert_eq!(
        scene.drawn_ambient().gradient,
        Some([a.sky, a.equator, a.ground])
    );
    assert_eq!(
        scene.drawn_ambient().color,
        scene.ambient_color,
        "the sky's colour"
    );
    let id = scene.add_entity("Ambient".into());
    let light = crate::components::LightComponent {
        light_type: LightType::Ambient,
        color: Vec3::new(1.0, 0.0, 0.0),
        intensity: 2.0,
        ..crate::scene::authoring::default_light()
    };
    scene.world.set_light(id, Some(light.clone()));
    let drawn = scene.drawn_ambient();
    assert_eq!(
        (drawn.color, drawn.intensity, drawn.gradient),
        (light.color, 2.0, None)
    );
}

#[test]
fn the_hemisphere_fades_to_a_quarter_and_the_trilight_meets_each_colour() {
    let c = Vec3::new(0.4, 0.8, 1.2);
    let hemi = AmbientRadiance::Hemisphere(c);
    assert_eq!(hemi.at(Vec3::Y), c);
    assert!(hemi.at(Vec3::NEG_Y).abs_diff_eq(c * 0.25, 1e-6));
    let (sky, equator, ground) = (Vec3::X, Vec3::Y, Vec3::Z);
    let tri = AmbientRadiance::Trilight {
        sky,
        equator,
        ground,
    };
    assert_eq!(tri.at(Vec3::Y), sky);
    assert_eq!(tri.at(Vec3::X), equator);
    assert_eq!(tri.at(Vec3::NEG_Y), ground);
    let down45 = Vec3::new(1.0, -1.0, 0.0).normalize();
    assert!(tri
        .at(down45)
        .abs_diff_eq(equator.lerp(ground, -down45.y), 1e-6));
    assert_eq!(tri.scaled(2.0).at(Vec3::Y), sky * 2.0);
}

/// The gradient's default colours are the default hemisphere's top, middle and
/// bottom, so switching the default scene to Gradient lights it the same.
#[test]
fn the_default_gradient_lights_as_the_default_colour() {
    let mut scene = Scene::new();
    let color = scene.drawn_ambient().radiance();
    scene.ambient.mode = AmbientMode::Gradient;
    let gradient = scene.drawn_ambient().radiance();
    for dir in crate::scene::lighting::probe_fill::fibonacci_sphere(32) {
        assert!(color.at(dir).abs_diff_eq(gradient.at(dir), 1e-6), "{dir}");
    }
}
