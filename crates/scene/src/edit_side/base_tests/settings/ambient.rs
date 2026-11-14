//! `crate::settings::ambient`: the ambient source through a save and load.

use glam::Vec3;

use crate::edit_side::serialize::{apply_scene_data, to_scene_data};
use crate::settings::ambient::{AmbientMode, AmbientSettings};
use crate::{Scene, SceneData};

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
