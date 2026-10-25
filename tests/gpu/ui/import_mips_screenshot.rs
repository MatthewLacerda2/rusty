//! A UI Image drawn smaller than its source reads its mip chain (#1397): a
//! one-texel checker minified 4× averages to grey with *Generate Mipmaps* on, and
//! keeps its black and white texels with it off. A sampler stuck at the base level
//! fails the first case. `Point` filtering keeps the base level free of blending.

use glam::Vec4;
use rusty::asset::sidecar::{FilterMode, TextureSettings};
use rusty::scene::Scene;

use super::super::render_texture_scene::screen_camera;
use super::import_filter_screenshot::checker_with;
use super::mask_scene::{element, overlay, px, shoot};

/// A 64² checker drawn into a 16-unit square at (16, 16), with or without mips:
/// how many of its pixels are grey (neither near black nor near white).
fn grey_pixels(mips: bool, name: &str) -> Option<usize> {
    let settings = TextureSettings {
        filter_mode: FilterMode::Point,
        generate_mipmaps: mips,
        ..Default::default()
    };
    let mut scene = Scene::new();
    let root = overlay(&mut scene);
    let id = element(&mut scene, root, [16.0, 16.0, 16.0, 16.0], Vec4::ONE);
    scene.world.image_mut(id).expect("image").texture = Some(checker_with(64, settings).into());
    let (img, _) = shoot(&scene, &screen_camera(), name)?;
    let grey = |(x, y): (u32, u32)| (40..216).contains(&px(&img, x, y)[0]);
    let inner = (17..31).flat_map(|y| (17..31).map(move |x| (x, y)));
    Some(inner.filter(|&p| grey(p)).count())
}

#[test]
fn a_minified_ui_image_reads_its_mips() {
    let Some(grey) = grey_pixels(true, "rusty_ui_mips_on.png") else {
        return;
    };
    assert!(grey >= 180, "with mips, 4× minified is grey: {grey}/196");
}

#[test]
fn a_minified_ui_image_without_mips_keeps_its_texels() {
    let Some(grey) = grey_pixels(false, "rusty_ui_mips_off.png") else {
        return;
    };
    assert!(
        grey <= 16,
        "without mips, texels stay black or white: {grey}/196"
    );
}
