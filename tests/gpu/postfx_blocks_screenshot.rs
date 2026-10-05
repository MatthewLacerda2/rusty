//! Sampling and animated postfx blocks (#402), pixel-tested on lavapipe:
//! `chromatic_aberration` splits a hard white/black edge into coloured fringes, and
//! `film_grain` paints different noise at two game times (and the same noise at
//! the same time — the effect is a pure function of pixel and clock).

use glam::Vec3;
use rusty::components::{MaterialAsset, MaterialComponent};
use rusty::dev::capture::CaptureHost;
use rusty::scene::Scene;
use rusty::shadergen::{bake_recipe, engine_shader_dir, ShaderRecipe, DEFAULT_OUT_DIR};

use super::fog_scene::{shot, wall_scene};

/// Bake a one-block postfx module named `name`.
fn bake(name: &str, block: &str) {
    let json = format!(r#"{{"pass":"postfx","name":"{name}","blocks":[{block}]}}"#);
    bake_recipe(
        &ShaderRecipe::from_json(&json).unwrap(),
        engine_shader_dir(),
        DEFAULT_OUT_DIR,
    )
    .expect("bake");
}

fn cleanup(name: &str) {
    std::fs::remove_file(format!("{DEFAULT_OUT_DIR}/{name}.wgsl")).ok();
}

fn with_effects(mut scene: Scene, effects: &[&str]) -> Scene {
    let id = scene.world.ids_with_visual_correction()[0];
    scene
        .world
        .visual_correction_mut(id)
        .unwrap()
        .custom_effects = effects.iter().map(|s| s.to_string()).collect();
    scene
}

/// A black backdrop with a white wall in front whose left edge sits right of the
/// screen centre, so the radial aberration offset there is non-zero.
fn edge_scene(effects: &[&str]) -> Scene {
    let mut scene = wall_scene(3.0, [0.0, 0.0, 0.0]);
    let back = scene.world.ids_with_mesh()[0];
    let mesh = scene.world.mesh(back).unwrap().clone();
    let white = scene.add_entity("White".to_string());
    // The box is 400 wide: centre it 200.5 right so its left edge is at x = 0.5.
    scene.world.transform_mut(white).unwrap().position = Vec3::new(200.5, 0.0, 3.0);
    scene.world.set_mesh(white, Some(mesh));
    scene.world.set_material(
        white,
        Some(MaterialComponent {
            material: "white".to_string(),
        }),
    );
    let asset = MaterialAsset {
        base_color: [0.0, 0.0, 0.0],
        emissive: [1.0, 1.0, 1.0],
        ..MaterialAsset::default()
    };
    scene.materials.insert("white".to_string(), asset);
    with_effects(scene, effects)
}

/// Whether the middle row holds a red fringe pixel and a yellow fringe pixel.
fn fringes(img: &image::RgbImage) -> (bool, bool) {
    let y = img.height() / 2;
    let row: Vec<[u8; 3]> = (0..img.width()).map(|x| img.get_pixel(x, y).0).collect();
    let red = row.iter().any(|&[r, g, b]| r > 150 && g < 60 && b < 60);
    let yellow = row.iter().any(|&[r, g, b]| r > 150 && g > 150 && b < 60);
    (red, yellow)
}

#[test]
fn chromatic_aberration_splits_a_hard_edge_into_fringes() {
    let name = "t402_aberration";
    bake(
        name,
        r#"{"id":"chromatic_aberration","params":{"strength":0.3}}"#,
    );
    let mut host = CaptureHost::new();
    let Some(plain) = shot(&mut host, &edge_scene(&[]), "pfx402_edge_plain") else {
        return;
    };
    assert_eq!(
        fringes(&plain),
        (false, false),
        "the plain edge has no fringes"
    );
    let split = shot(&mut host, &edge_scene(&[name]), "pfx402_edge_split").unwrap();
    assert_eq!(fringes(&split), (true, true), "red and yellow fringes");
    cleanup(name);
}

#[test]
fn film_grain_changes_with_game_time() {
    let name = "t402_grain";
    bake(name, r#"{"id":"film_grain","params":{"strength":0.5}}"#);
    let mut host = CaptureHost::new();
    let at = |host: &mut CaptureHost, time: f32, tag: &str| {
        let mut scene = with_effects(wall_scene(2.0, [0.5, 0.5, 0.5]), &[name]);
        scene.shader_time = time;
        shot(host, &scene, tag)
    };
    let Some(a) = at(&mut host, 1.0, "pfx402_grain_a") else {
        return;
    };
    let b = at(&mut host, 2.0, "pfx402_grain_b").unwrap();
    let again = at(&mut host, 1.0, "pfx402_grain_a2").unwrap();
    let differing = a.pixels().zip(b.pixels()).filter(|(p, q)| p != q).count();
    let total = (a.width() * a.height()) as usize;
    assert!(
        differing > total / 2,
        "only {differing}/{total} pixels changed"
    );
    assert_eq!(a, again, "the same game time paints the same grain");
    cleanup(name);
}
