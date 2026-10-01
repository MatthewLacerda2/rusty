//! Render textures (#430): which are worth drawing, and when.

use super::slots::is_due;
use super::*;
use crate::components::{ImageComponent, MaterialAsset, MaterialComponent};

#[test]
fn only_rt_prefixed_paths_are_render_textures() {
    assert!(is_render_texture("rt:minimap"));
    assert!(!is_render_texture("assets/textures/rt:not.png"));
    assert!(!is_render_texture("minimap"));
}

#[test]
fn update_every_spaces_the_draws() {
    // Never drawn: due at once, whatever the rate.
    assert!(is_due(None, 7, 4));
    // Every frame.
    assert!(is_due(Some(7), 8, 1));
    // Every 4th: not on the next three frames, then due.
    assert!(!is_due(Some(8), 9, 4));
    assert!(!is_due(Some(8), 11, 4));
    assert!(is_due(Some(8), 12, 4));
    // 0 reads as every frame, never as "never".
    assert!(is_due(Some(8), 9, 0));
}

#[test]
fn referenced_textures_come_from_active_images_and_material_maps() {
    let mut scene = Scene::new();
    let img = scene.add_entity("Minimap".to_string());
    let image = ImageComponent {
        texture: Some("rt:minimap".to_string()),
        ..Default::default()
    };
    scene.world.set_image(img, Some(image));

    let hidden = scene.add_entity("Hidden".to_string());
    let image = ImageComponent {
        texture: Some("rt:hidden".to_string()),
        ..Default::default()
    };
    scene.world.set_image(hidden, Some(image));
    scene.world.set_active(hidden, false);

    let file = scene.add_entity("Icon".to_string());
    let image = ImageComponent {
        texture: Some("assets/icon.png".to_string()),
        ..Default::default()
    };
    scene.world.set_image(file, Some(image));

    let monitor = scene.add_entity("Monitor".to_string());
    let material = MaterialComponent {
        material: "screen".to_string(),
    };
    scene.world.set_material(monitor, Some(material));
    let asset = MaterialAsset {
        emissive_map: Some("rt:security".to_string()),
        // An extra shader texture slot (#400) may name one too.
        shader_textures: [("mask".to_string(), "rt:scanner".to_string())].into(),
        ..Default::default()
    };
    scene.materials.insert("screen".to_string(), asset);

    let shown = referenced_render_textures(&scene);
    let mut shown: Vec<_> = shown.into_iter().collect();
    shown.sort();
    assert_eq!(shown, ["rt:minimap", "rt:scanner", "rt:security"]);
}
