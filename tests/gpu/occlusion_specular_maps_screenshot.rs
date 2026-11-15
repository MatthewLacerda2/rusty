//! The occlusion and specular-level maps (#2151). Each check renders a plane with a
//! map against the same plane without one (or with the scalar it stands for):
//!   - Occlusion: a black map darkens the ambient-lit plane, a strength of 0 leaves
//!     it as unmapped, and under a lamp alone (no ambient, no reflectance) it changes
//!     nothing: direct light is never occluded by it.
//!   - Specular level: a map whose alpha is 0 shades like `specular = 0`, and unlike
//!     the unmapped plane, whose lamp highlight it takes away.
//!
//! No adapter: `frame` returns `None` and the test passes without asserting.

use super::linear_data_maps_screenshot::{diff, frame, png, scene as lamp_scene, SAME};
use super::normal_emissive_maps_screenshot::scene as ambient_scene;
use rusty::components::MaterialAsset;
use rusty::scene::Scene;

/// Render `scenes` and compare the first with each of the rest; `None` without a GPU.
fn diffs(what: &str, scenes: Vec<Scene>) -> Option<Vec<f64>> {
    let frames: Option<Vec<_>> = (scenes.iter().enumerate())
        .map(|(i, s)| frame(s, &format!("rusty_2151_{what}_{i}.png")))
        .collect();
    let Some(frames) = frames else {
        eprintln!("[occlusion-specular] no GPU adapter — skipping {what}");
        return None;
    };
    let out: Vec<f64> = frames[1..].iter().map(|f| diff(&frames[0], f)).collect();
    eprintln!("[occlusion-specular] {what}: {out:?}");
    Some(out)
}

/// A rough grey dielectric, with `edit` applied.
fn grey(edit: impl FnOnce(&mut MaterialAsset)) -> MaterialAsset {
    let mut m = MaterialAsset {
        base_color: [0.8, 0.8, 0.8],
        roughness: 0.9,
        ..MaterialAsset::default()
    };
    edit(&mut m);
    m
}

#[test]
fn a_black_occlusion_map_darkens_the_indirect_light_and_never_a_lamp() {
    let black = png("rusty_2151_ao_black.png", [0, 0, 0]);
    let occluded = |strength| {
        grey(|m| {
            m.occlusion_map = Some(black.clone().into());
            m.occlusion_strength = strength;
        })
    };
    let ambient = [grey(|_| {}), occluded(1.0), occluded(0.0)].map(ambient_scene);
    let Some(d) = diffs("ambient", ambient.into()) else {
        return;
    };
    assert!(d[0] > 10.0, "a black map darkens the ambient: {:.3}", d[0]);
    assert!(d[1] < SAME, "strength 0 applies none of it: {:.3}", d[1]);
    // A lamp alone: no ambient, and no reflectance (`specular` 0) for the
    // occlusion's specular term to take.
    let lit = |m: MaterialAsset| {
        let mut s = lamp_scene(MaterialAsset { specular: 0.0, ..m });
        s.ambient_intensity = 0.0;
        s
    };
    let Some(d) = diffs("lamp", vec![lit(grey(|_| {})), lit(occluded(1.0))]) else {
        return;
    };
    assert!(d[0] < SAME, "direct light is not occluded: {:.3}", d[0]);
}

#[test]
fn a_zero_specular_level_map_shades_like_specular_zero() {
    // RGB white, alpha 0: the level is read from alpha.
    let path = crate::temp::dir().join("rusty_2151_level_zero.png");
    image::RgbaImage::from_pixel(2, 2, image::Rgba([255, 255, 255, 0]))
        .save(&path)
        .expect("write png");
    let glossy = |edit: fn(&mut MaterialAsset), map: Option<&std::path::Path>| {
        grey(|m| {
            m.roughness = 0.3;
            m.specular_map = map.map(|p| p.to_string_lossy().into_owned().into());
            edit(m);
        })
    };
    let scenes = [
        glossy(|_| {}, Some(&path)),
        glossy(|m| m.specular = 0.0, None),
        glossy(|_| {}, None),
    ];
    let Some(d) = diffs("level", scenes.map(lamp_scene).into()) else {
        return;
    };
    assert!(d[0] < SAME, "alpha 0 reads as specular 0: {:.3}", d[0]);
    assert!(d[1] > SAME, "and takes the highlight away: {:.3}", d[1]);
}
