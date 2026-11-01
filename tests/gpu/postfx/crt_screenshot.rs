//! The CRT postfx blocks (#1679), pixel-tested on lavapipe: `barrel` warps where
//! the frame is sampled, so a colour block after it still grades the picture inside
//! the curve while the corners go black; `phosphor_mask` stripes a flat grey without
//! changing its mean brightness (short of clipping).

use rusty::dev::capture::CaptureHost;
use rusty::scene::Scene;
use rusty::shadergen::{bake_recipe, engine_shader_dir, ShaderRecipe, DEFAULT_OUT_DIR};

use super::super::fog::scene::{shot, wall_scene};

/// Bake a postfx module named `name` from its JSON `blocks` list.
fn bake(name: &str, blocks: &str) {
    let json = format!(r#"{{"pass":"postfx","name":"{name}","blocks":[{blocks}]}}"#);
    bake_recipe(
        &rusty::api::recipe_from_json_str::<ShaderRecipe>(&json).unwrap(),
        engine_shader_dir(),
        DEFAULT_OUT_DIR,
    )
    .expect("bake");
}

/// Removes a baked module's files when dropped, so they go even if an assertion fails.
struct Cleanup(String);

impl Drop for Cleanup {
    fn drop(&mut self) {
        for ext in ["wgsl", "params.json", "recipe.json"] {
            std::fs::remove_file(format!("{DEFAULT_OUT_DIR}/{}.{ext}", self.0)).ok();
        }
    }
}

/// A flat grey wall filling the view, seen through `effects`: a dim one, so the
/// mask's lit stripe stays under 1 and doesn't clip.
fn grey(effects: &[&str]) -> Scene {
    let mut scene = wall_scene(2.0, [0.2, 0.2, 0.2]);
    let id = scene.world.ids_with_post_processing()[0];
    scene
        .world
        .post_processing_mut(id)
        .unwrap()
        .custom_effects
        .names = effects.iter().map(|s| s.to_string()).collect();
    scene
}

/// The mean display value of each channel over the whole image: the chain runs on
/// display-referred colour, so that is the mean the mask keeps.
fn mean(img: &image::RgbImage) -> [f64; 3] {
    let n = f64::from(img.width() * img.height());
    let mut sum = [0.0; 3];
    for p in img.pixels() {
        for (s, c) in sum.iter_mut().zip(p.0) {
            *s += f64::from(c) / 255.0;
        }
    }
    sum.map(|s| s / n)
}

#[test]
fn barrel_then_tint_tints_inside_the_curve_and_blacks_the_corners() {
    let name = "t1679_barrel_tint";
    let _cleanup = Cleanup(name.into());
    bake(
        name,
        r#"{"id":"barrel","params":{"amount":0.2,"corner_radius":0.1}},{"id":"tint","params":{"color":[1,0,0]}}"#,
    );
    let mut host = CaptureHost::new();
    let Some(img) = shot(&mut host, &grey(&[name]), "pfx1679_barrel") else {
        return;
    };
    let (w, h) = (img.width(), img.height());
    let centre = img.get_pixel(w / 2, h / 2).0;
    assert!(
        centre[0] > 100 && centre[1] < 4 && centre[2] < 4,
        "tinted red inside: {centre:?}"
    );
    for (x, y) in [(0, 0), (w - 1, 0), (0, h - 1), (w - 1, h - 1)] {
        assert_eq!(
            img.get_pixel(x, y).0,
            [0, 0, 0],
            "corner ({x}, {y}) is black"
        );
    }
    // Mid-edge the curve pulls the frame from past its edge: black there too, and the
    // picture still reaches a few pixels in.
    assert_eq!(
        img.get_pixel(0, h / 2).0,
        [0, 0, 0],
        "the left edge is past the curve"
    );
    assert!(
        img.get_pixel(w / 4, h / 2).0[0] > 100,
        "the picture reaches in from the edge"
    );
}

#[test]
fn phosphor_mask_stripes_a_flat_grey_and_keeps_its_mean() {
    let mut host = CaptureHost::new();
    let Some(plain) = shot(&mut host, &grey(&[]), "pfx1679_plain") else {
        return;
    };
    let before = mean(&plain);
    for kind in ["aperture", "shadow", "slot"] {
        let name = format!("t1679_mask_{kind}");
        let _cleanup = Cleanup(name.clone());
        bake(
            &name,
            &format!(r#"{{"id":"phosphor_mask","params":{{"kind":"{kind}"}}}}"#),
        );
        let img = shot(&mut host, &grey(&[&name]), &format!("pfx1679_{kind}")).unwrap();
        let (a, b) = (img.get_pixel(10, 10).0, img.get_pixel(11, 10).0);
        assert_ne!(a, b, "{kind}: neighbouring pixels differ (stripes)");
        let after = mean(&img);
        for c in 0..3 {
            let drift = (after[c] - before[c]).abs() / before[c];
            assert!(
                drift < 0.05,
                "{kind} channel {c}: mean {} → {}",
                before[c],
                after[c]
            );
        }
    }
}
