//! Sampling postfx blocks (#402), pixel-tested on lavapipe: `radial_blur` taps the
//! module's input at offsets, smearing a hard white/black edge into greys. (The
//! lens effects these tests once covered as blocks are the volume's now, #1439:
//! `lens_screenshot`.)

use glam::Vec3;
use rusty::components::{MaterialAsset, MaterialComponent};
use rusty::dev::capture::CaptureHost;
use rusty::scene::Scene;
use rusty::shadergen::{bake_recipe, engine_shader_dir, ShaderRecipe, DEFAULT_OUT_DIR};

use super::super::fog::scene::{shot, wall_scene};

/// Bake a one-block postfx module named `name`.
fn bake(name: &str, block: &str) {
    let json = format!(r#"{{"pass":"postfx","name":"{name}","blocks":[{block}]}}"#);
    bake_recipe(
        &rusty::api::recipe_from_json_str::<ShaderRecipe>(&json).unwrap(),
        engine_shader_dir(),
        DEFAULT_OUT_DIR,
    )
    .expect("bake");
}

fn cleanup(name: &str) {
    std::fs::remove_file(format!("{DEFAULT_OUT_DIR}/{name}.wgsl")).ok();
}

fn with_effects(mut scene: Scene, effects: &[&str]) -> Scene {
    let id = scene.world.ids_with_post_processing()[0];
    scene.world.post_processing_mut(id).unwrap().custom_effects =
        effects.iter().map(|s| s.to_string()).collect();
    scene
}

/// A black backdrop with a white wall in front whose left edge sits right of the
/// screen centre, so the radial blur offset there is non-zero.
fn edge_scene(effects: &[&str]) -> Scene {
    let mut scene = wall_scene(3.0, [0.0, 0.0, 0.0]);
    let back = scene.world.ids_with_mesh()[0];
    let mesh = scene.world.mesh(back).unwrap().clone();
    let white = scene.add_entity("White".to_string());
    // The box is 400 wide: centre it 200.5 right so its left edge is at x = 0.5.
    scene.world.transform_mut(white).unwrap().position = Vec3::new(200.5, 0.0, 3.0);
    scene.world.set_mesh(white, Some(mesh));
    scene
        .world
        .set_material(white, Some(MaterialComponent::new("white".to_string())));
    let asset = MaterialAsset {
        base_color: [0.0, 0.0, 0.0],
        emissive: [1.0, 1.0, 1.0],
        ..MaterialAsset::default()
    };
    scene.materials.insert("white".to_string(), asset);
    with_effects(scene, effects)
}

/// How many middle-row pixels are neither black nor white: the smeared edge.
fn greys(img: &image::RgbImage) -> usize {
    let y = img.height() / 2;
    (0..img.width())
        .filter(|&x| (40..215).contains(&img.get_pixel(x, y).0[1]))
        .count()
}

#[test]
fn radial_blur_smears_a_hard_edge() {
    let name = "t402_radial_blur";
    bake(name, r#"{"id":"radial_blur","params":{"strength":0.3}}"#);
    let mut host = CaptureHost::new();
    let Some(plain) = shot(&mut host, &edge_scene(&[]), "pfx402_edge_plain") else {
        return;
    };
    let blurred = shot(&mut host, &edge_scene(&[name]), "pfx402_edge_blur").unwrap();
    let (before, after) = (greys(&plain), greys(&blurred));
    assert!(
        after >= before + 3,
        "{before} grey pixels before, {after} after"
    );
    cleanup(name);
}
