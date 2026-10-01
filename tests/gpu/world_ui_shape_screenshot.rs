//! Shapes on a world canvas (#425): the SDF graphic draws in perspective through
//! the world pass, respects a RectMask's per-fragment clip there (#619), and the
//! world pass has a pipeline per blend mode.

use glam::{Vec2, Vec4};
use rusty::components::{
    CanvasComponent, CanvasRenderMode, RectMaskComponent, ShapeComponent, ShapeGlow, UiBlend,
};
use rusty::scene::Scene;

use super::world_ui_scene::{canvas, dark_scene, dominant, fill, image};
use super::world_ui_screenshot::shot;

/// Put a shape of `color` and `blend`, glowing `glow` units, on `id`.
fn skin(scene: &mut Scene, id: u32, color: Vec4, blend: UiBlend, glow: f32) {
    scene.world.set_image(id, None);
    let shape = ShapeComponent {
        color,
        blend,
        glow: ShapeGlow {
            size: glow,
            intensity: 1.0,
            color,
        },
        ..Default::default()
    };
    scene.world.set_shape(id, Some(shape));
}

/// A 2×2 m, 400-unit sign 5 m ahead (pixels ~25..71 on both axes) on black: its
/// left half masks a red shape twice its size (glow and all), its right half
/// carries an additive green shape.
fn scene() -> Scene {
    let mut scene = dark_scene();
    let sign = CanvasComponent {
        render_mode: CanvasRenderMode::WorldSpace,
        reference_resolution: Vec2::splat(400.0),
        pixels_per_unit: 200.0,
        ..Default::default()
    };
    let sign = canvas(&mut scene, sign);
    // A black backdrop, so the additive result is measured against a known colour.
    image(&mut scene, sign, fill(), Vec4::new(0.0, 0.0, 0.0, 1.0));
    // The left half masks a red shape twice its size (glow and all).
    let mut half = fill();
    half.anchor_max = Vec2::new(0.5, 1.0);
    let mask = image(&mut scene, sign, half, Vec4::ZERO);
    scene
        .world
        .set_rect_mask(mask, Some(RectMaskComponent::default()));
    let mut over = fill();
    over.anchor_max = Vec2::splat(2.0);
    let red = image(&mut scene, mask, over, Vec4::ONE);
    skin(
        &mut scene,
        red,
        Vec4::new(1.0, 0.0, 0.0, 1.0),
        UiBlend::Normal,
        40.0,
    );
    // The right half carries an additive green shape.
    let mut right = fill();
    right.anchor_min = Vec2::new(0.6, 0.2);
    right.anchor_max = Vec2::new(0.9, 0.8);
    let green = image(&mut scene, sign, right, Vec4::ONE);
    skin(
        &mut scene,
        green,
        Vec4::new(0.0, 0.4, 0.0, 1.0),
        UiBlend::Additive,
        0.0,
    );
    scene
}

#[test]
fn a_masked_shape_and_an_additive_shape_draw_on_a_world_canvas() {
    let Some(img) = shot(&scene(), "rusty_world_ui_shape.png") else {
        return;
    };
    let px = |x: u32, y: u32| img.get_pixel(x, y).0;
    assert!(
        dominant(px(36, 48), 0),
        "the shape inside the mask: {:?}",
        px(36, 48)
    );
    assert!(
        !dominant(px(36, 10), 0),
        "clipped above the sign: {:?}",
        px(36, 10)
    );
    assert!(
        dominant(px(62, 48), 1),
        "the additive shape: {:?}",
        px(62, 48)
    );
}
