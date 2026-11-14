//! `crate::settings`: the scene-wide settings through a save and load.

mod ambient;

use glam::Vec3;

use crate::create::default_light;
use crate::edit_side::serialize::{apply_scene_data, to_scene_data};
use crate::{AmbientMode, FogMode, LightType, Scene, SceneData};

#[test]
fn fog_saves_and_loads_with_the_scene() {
    let mut scene = Scene::new();
    scene.fog.mode = FogMode::Linear;
    scene.fog.end = 42.0;
    let json = serde_json::to_string(&to_scene_data(&scene)).unwrap();
    let mut back = Scene::new();
    apply_scene_data(&mut back, serde_json::from_str::<SceneData>(&json).unwrap());
    assert_eq!(back.fog, scene.fog);
}

#[test]
fn the_sky_saves_and_loads_with_the_scene() {
    let mut scene = Scene::new();
    scene.sky.cloud_coverage = 0.9;
    scene.sky.ground = Some(Vec3::new(0.2, 0.15, 0.1));
    let json = serde_json::to_string(&to_scene_data(&scene)).unwrap();
    let mut back = Scene::new();
    apply_scene_data(&mut back, serde_json::from_str::<SceneData>(&json).unwrap());
    assert_eq!(back.sky, scene.sky);
}

#[test]
fn an_untouched_sky_stays_out_of_the_file() {
    let scene = Scene::new();
    let json = serde_json::to_string(&to_scene_data(&scene)).unwrap();
    assert!(!json.contains("\"sky\""), "{json}");
}

#[test]
fn an_ambient_light_overrides_the_sky_and_the_mode_saves() {
    let mut scene = Scene::new();
    scene.ambient.mode = AmbientMode::Skybox;
    let id = scene.add_entity("Ambient".into());
    let light = rusty_components::LightComponent {
        light_type: LightType::Ambient,
        color: Vec3::X,
        ..default_light()
    };
    scene.world.set_light(id, Some(light));
    let drawn = scene.drawn_ambient();
    assert_eq!((drawn.skybox, drawn.color), (None, Vec3::X));
    let json = serde_json::to_string(&to_scene_data(&scene)).unwrap();
    assert!(
        json.contains("\"Skybox\"") && !json.contains("sky_sh"),
        "{json}"
    );
}
