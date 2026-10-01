//! Frosted-glass backdrop (#426), headless: a BackdropFilter panel over a world
//! split sharply into red and blue blurs the split under it and nowhere else,
//! its filter greys and darkens what it shows, and no backdrop means no blur
//! passes. `px` takes UI coordinates (bottom-left origin).

use glam::{Vec3, Vec4};
use rusty::components::BackdropFilterComponent;
use rusty::scene::{Camera, Scene};

use super::ui_mask_scene::{element, overlay, shoot, SIZE};
use super::world_ui_scene::{dark_scene, emissive_box};

fn px(img: &image::RgbImage, x: u32, y: u32) -> [u8; 3] {
    img.get_pixel(x, SIZE - 1 - y).0
}

/// Red left of x = 0, blue right of it, filling the view of [`camera`].
fn split_world() -> Scene {
    let mut scene = dark_scene();
    let (size, red, blue) = (Vec3::new(10.0, 20.0, 0.2), [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]);
    emissive_box(&mut scene, "Red", Vec3::new(-5.0, 0.0, 0.0), size, red);
    emissive_box(&mut scene, "Blue", Vec3::new(5.0, 0.0, 0.0), size, blue);
    scene
}

fn camera() -> Camera {
    Camera::new(Vec3::new(0.0, 0.0, 5.0), -90.0, 0.0)
}

/// A glass panel over the canvas's top half (y 64..128), its own colour clear.
fn glass(scene: &mut Scene, filter: BackdropFilterComponent) -> u32 {
    let root = overlay(scene);
    let panel = element(scene, root, [0.0, 64.0, 128.0, 64.0], Vec4::ZERO);
    scene.world.set_backdrop_filter(panel, Some(filter));
    panel
}

#[test]
fn a_backdrop_blurs_the_frame_under_its_panel_only() {
    let mut scene = split_world();
    let filter = BackdropFilterComponent {
        blur_radius: 24.0,
        ..Default::default()
    };
    glass(&mut scene, filter);
    let Some((img, counters)) = shoot(&scene, &camera(), "rusty_ui_backdrop.png") else {
        return;
    };
    // Below the panel the split is sharp: a few pixels left of it is pure red.
    let sharp = px(&img, 60, 32);
    assert!(sharp[0] > 200 && sharp[2] < 30, "unblurred: {sharp:?}");
    // Under the panel the same column has blue blurred into it, and vice versa.
    let (left, right) = (px(&img, 60, 96), px(&img, 68, 96));
    assert!(
        left[0] > 60 && left[2] > 60,
        "red side mixes in blue: {left:?}"
    );
    assert!(
        right[0] > 60 && right[2] > 60,
        "blue side mixes in red: {right:?}"
    );
    // Far from the split the blur keeps the colour.
    let far = px(&img, 8, 96);
    assert!(far[0] > 180 && far[2] < 60, "far left stays red: {far:?}");
    assert!(counters.ui_blur_passes >= 2, "{}", counters.ui_blur_passes);
    assert_eq!(counters.ui_mask_passes, 0);
}

#[test]
fn the_filter_greys_and_darkens_the_backdrop() {
    let mut scene = split_world();
    let filter = BackdropFilterComponent {
        blur_radius: 0.0,
        saturation: 0.0,
        brightness: 0.5,
        ..Default::default()
    };
    let panel = glass(&mut scene, filter);
    let Some((img, counters)) = shoot(&scene, &camera(), "rusty_ui_backdrop_grey.png") else {
        return;
    };
    let grey = px(&img, 16, 96);
    let spread = grey.iter().max().unwrap() - grey.iter().min().unwrap();
    assert!(spread < 8, "greyscale: {grey:?}");
    assert!(grey[0] > 10 && grey[0] < 60, "red's luma, halved: {grey:?}");
    assert_eq!(counters.ui_blur_passes, 1, "level 0: the composite only");

    scene.world.set_backdrop_filter(panel, None);
    let Some((img, counters)) = shoot(&scene, &camera(), "rusty_ui_no_backdrop.png") else {
        return;
    };
    assert!(
        px(&img, 16, 96)[0] > 200,
        "no filter, the frame shows through"
    );
    assert_eq!(counters.ui_blur_passes, 0);
}

#[test]
fn a_small_panel_is_glass_only_inside_its_rect() {
    let mut scene = split_world();
    let root = overlay(&mut scene);
    let panel = element(&mut scene, root, [48.0, 48.0, 32.0, 32.0], Vec4::ZERO);
    let filter = BackdropFilterComponent {
        blur_radius: 16.0,
        ..Default::default()
    };
    scene.world.set_backdrop_filter(panel, Some(filter));
    let Some((img, _)) = shoot(&scene, &camera(), "rusty_ui_backdrop_small.png") else {
        return;
    };
    let inside = px(&img, 62, 64);
    assert!(inside[2] > 40, "blurred inside: {inside:?}");
    let beside = px(&img, 62, 40);
    assert!(beside[2] < 30, "sharp just below it: {beside:?}");
}
