//! Soft and shaped UI masks (#428), headless: the headline round minimap — a
//! render texture shown through a circular soft Mask — a feathered scroll list,
//! and two nested Masks multiplying. Asserted at known pixels (`px` takes UI
//! coordinates: bottom-left origin). Child rects are placed relative to their
//! parent's bottom-left corner.

use glam::Vec4;
use rusty::components::{ImageComponent, RectMaskComponent, ShapeComponent, ShapeKind};
use rusty::scene::Scene;

use super::render_texture_scene::screen_camera;
use super::ui_mask_scene::SIZE;
use super::ui_mask_scene::{element, hidden_mask, minimap_world, overlay, shoot, soft_circle};

const WHITE: Vec4 = Vec4::ONE;
const BLACK: Vec4 = Vec4::new(0.0, 0.0, 0.0, 1.0);

/// A scene whose overlay canvas starts with an opaque black backdrop.
fn black_canvas() -> (Scene, u32) {
    let mut scene = Scene::new();
    let root = overlay(&mut scene);
    element(&mut scene, root, [0.0, 0.0, 128.0, 128.0], BLACK);
    (scene, root)
}

fn px(img: &image::RgbImage, x: u32, y: u32) -> [u8; 3] {
    img.get_pixel(x, SIZE - 1 - y).0
}

fn is_red(p: [u8; 3]) -> bool {
    p[0] > 150 && p[1] < 60 && p[2] < 60
}

#[test]
fn a_minimap_shows_its_render_texture_through_a_circular_soft_mask() {
    let mut scene = minimap_world();
    let root = overlay(&mut scene);
    // A 96-unit circle frame at (16, 16), its edge fading over 8 texels; centre (64, 64).
    let frame = element(&mut scene, root, [16.0, 16.0, 96.0, 96.0], WHITE);
    let circle = ImageComponent {
        texture: Some(soft_circle(96, 8.0)),
        ..Default::default()
    };
    scene.world.set_image(frame, Some(circle));
    hidden_mask(&mut scene, frame);
    let map = element(&mut scene, frame, [0.0, 0.0, 96.0, 96.0], WHITE);
    scene.world.image_mut(map).expect("image").texture = Some("rt:minimap".into());
    let Some((img, counters)) = shoot(&scene, &screen_camera(), "rusty_ui_minimap.png") else {
        return;
    };
    assert!(is_red(px(&img, 64, 64)), "centre: {:?}", px(&img, 64, 64));
    assert!(
        is_red(px(&img, 64, 104)),
        "near the top edge: {:?}",
        px(&img, 64, 104)
    );
    // The frame's corner is inside its square but outside the circle.
    let corner = px(&img, 20, 20);
    assert!(!is_red(corner), "masked-out corner (the sky): {corner:?}");
    // The edge is soft: half a fade in from the rim is partly red.
    let (rim, inside) = (px(&img, 64, 16 + 4), px(&img, 64, 16 + 12));
    assert!(
        !is_red(rim) && is_red(inside),
        "the rim fades: {rim:?} {inside:?}"
    );
    assert!(rim[1] < inside[1] + 120, "half sky, half red: {rim:?}");
    assert_eq!(counters.render_texture_draws, 1);
    assert_eq!(counters.ui_mask_passes, 1);
    assert_eq!(counters.ui_blur_passes, 0, "no backdrop, no blur");
}

#[test]
fn a_feathered_rect_mask_fades_a_list_at_its_edges() {
    let (mut scene, root) = black_canvas();
    // A viewport over y 32..96 feathered by 16; its content overflows it.
    let view = element(&mut scene, root, [0.0, 32.0, 128.0, 64.0], Vec4::ZERO);
    let soft = RectMaskComponent {
        feather: 16.0,
        ..Default::default()
    };
    scene.world.set_rect_mask(view, Some(soft));
    element(&mut scene, view, [0.0, 0.0, 128.0, 128.0], WHITE);
    let Some((img, counters)) = shoot(&scene, &screen_camera(), "rusty_ui_feather.png") else {
        return;
    };
    let level = |y| i32::from(px(&img, 64, y)[0]);
    assert!(level(64) > 240, "the middle is solid: {}", level(64));
    // 8 of 16 feather pixels in: about half (blended in display space).
    assert!((100..=156).contains(&level(40)), "half way: {}", level(40));
    assert!(level(33) < level(40) && level(40) < level(50), "a ramp");
    assert!(level(20) < 8, "outside the viewport: {}", level(20));
    assert_eq!(
        counters.ui_mask_passes, 0,
        "a RectMask needs no mask texture"
    );
}

#[test]
fn nested_masks_multiply() {
    let (mut scene, root) = black_canvas();
    // A circle of radius 48 around (64, 64), then a rect Mask (no Image) over the
    // right half of the canvas inside it.
    let outer = element(&mut scene, root, [16.0, 16.0, 96.0, 96.0], WHITE);
    let circle = ImageComponent {
        texture: Some(soft_circle(96, 1.0)),
        ..Default::default()
    };
    scene.world.set_image(outer, Some(circle));
    hidden_mask(&mut scene, outer);
    let inner = element(&mut scene, outer, [48.0, -16.0, 64.0, 128.0], WHITE);
    scene.world.set_image(inner, None);
    hidden_mask(&mut scene, inner);
    element(&mut scene, inner, [-64.0, 0.0, 128.0, 128.0], WHITE);
    let Some((img, counters)) = shoot(&scene, &screen_camera(), "rusty_ui_nested.png") else {
        return;
    };
    let lit = |x, y| px(&img, x, y)[0] > 200;
    assert!(lit(90, 64), "inside both");
    assert!(!lit(40, 64), "in the circle, left of the rect");
    assert!(!lit(124, 64), "in the rect, outside the circle");
    assert!(!lit(70, 122), "in the rect, above the circle");
    assert_eq!(counters.ui_mask_passes, 2);
}

#[test]
fn a_shape_ellipse_masks_without_a_sprite() {
    let (mut scene, root) = black_canvas();
    let frame = element(&mut scene, root, [16.0, 16.0, 96.0, 96.0], WHITE);
    scene.world.set_image(frame, None);
    let ellipse = ShapeComponent {
        kind: ShapeKind::Ellipse,
        ..Default::default()
    };
    scene.world.set_shape(frame, Some(ellipse));
    hidden_mask(&mut scene, frame);
    element(&mut scene, frame, [0.0, 0.0, 96.0, 96.0], WHITE);
    let Some((img, _)) = shoot(&scene, &screen_camera(), "rusty_ui_shape_mask.png") else {
        return;
    };
    assert!(px(&img, 64, 64)[0] > 240, "centre");
    assert!(px(&img, 64, 108)[0] > 200, "inside the top of the ellipse");
    assert!(px(&img, 22, 22)[0] < 16, "the frame's corner is outside it");
}
