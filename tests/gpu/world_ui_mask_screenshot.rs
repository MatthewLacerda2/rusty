//! RectMask on a world canvas (#619): a graphic overflowing its mask is cut at
//! the mask's edge, per fragment, though the canvas is a plane in perspective —
//! and so a world-space Scroll View clips its content to its viewport. A graphic
//! `Mask` (#428) clips there too, through the same coverage texture as on screen.

use glam::{Vec2, Vec4};
use rusty::components::{
    CanvasComponent, CanvasRenderMode, LayoutElementComponent, MaskComponent, RectMaskComponent,
    ShapeComponent, ShapeKind,
};
use rusty::scene::authoring::ui_widgets::{create_ui, UiWidget};
use rusty::scene::Scene;

use crate::ui_widgets::part;

use super::world_ui_scene::{canvas, dark_scene, dominant, fill, image};
use super::world_ui_screenshot::shot;

const RED: Vec4 = Vec4::new(1.0, 0.0, 0.0, 1.0);

/// A 2×2 m, 400-unit sign at the origin, 5 m in front of the camera: it spans
/// pixels ~25..71 on both axes, its centre at 48.
fn sign(scene: &mut Scene) -> u32 {
    let sign = CanvasComponent {
        render_mode: CanvasRenderMode::WorldSpace,
        reference_resolution: Vec2::splat(400.0),
        pixels_per_unit: 200.0,
        ..Default::default()
    };
    canvas(scene, sign)
}

#[test]
fn a_child_overflowing_its_mask_is_cut_at_the_mask_edge() {
    let mut scene = dark_scene();
    let sign = sign(&mut scene);
    // The mask is the sign's left half (fully transparent itself).
    let mut half = fill();
    half.anchor_max = Vec2::new(0.5, 1.0);
    let mask = image(&mut scene, sign, half, Vec4::ZERO);
    scene
        .world
        .set_rect_mask(mask, Some(RectMaskComponent::default()));
    // Its child spans twice the mask both ways: the whole sign and beyond its top.
    let mut over = fill();
    over.anchor_max = Vec2::splat(2.0);
    image(&mut scene, mask, over, RED);
    let Some(img) = shot(&scene, "rusty_world_ui_mask.png") else {
        return;
    };
    let px = |x: u32, y: u32| img.get_pixel(x, y).0;
    assert!(dominant(px(36, 48), 0), "inside the mask: {:?}", px(36, 48));
    assert!(!dominant(px(60, 48), 0), "right of it: {:?}", px(60, 48));
    assert!(!dominant(px(36, 10), 0), "above the sign: {:?}", px(36, 10));
}

#[test]
fn a_world_space_scroll_view_clips_its_content_to_the_viewport() {
    let mut scene = dark_scene();
    let sign = sign(&mut scene);
    let view = create_ui(&mut scene, UiWidget::ScrollView, Some(sign));
    let content = part(&scene, view, "Viewport/Content");
    // A 600-tall red item in a 380-tall viewport: 220 units hang below the sign.
    let item = image(&mut scene, content, fill(), RED);
    let tall = LayoutElementComponent {
        preferred_height: Some(600.0),
        ..Default::default()
    };
    scene.world.set_layout_element(item, Some(tall));
    let Some(img) = shot(&scene, "rusty_world_ui_scroll_view.png") else {
        return;
    };
    let px = |x: u32, y: u32| img.get_pixel(x, y).0;
    assert!(dominant(px(36, 48), 0), "in the viewport: {:?}", px(36, 48));
    assert!(!dominant(px(36, 85), 0), "below the sign: {:?}", px(36, 85));
}

#[test]
fn a_shape_mask_rounds_a_world_canvas() {
    let mut scene = dark_scene();
    let sign = sign(&mut scene);
    // The whole sign is an ellipse Mask (its graphic hidden) over a red fill.
    let frame = image(&mut scene, sign, fill(), Vec4::ZERO);
    scene.world.set_image(frame, None);
    let ellipse = ShapeComponent {
        kind: ShapeKind::Ellipse,
        ..Default::default()
    };
    scene.world.set_shape(frame, Some(ellipse));
    let hidden = MaskComponent {
        show_mask_graphic: false,
    };
    scene.world.set_mask(frame, Some(hidden));
    image(&mut scene, frame, fill(), RED);
    let Some(img) = shot(&scene, "rusty_world_ui_shape_mask.png") else {
        return;
    };
    let px = |x: u32, y: u32| img.get_pixel(x, y).0;
    assert!(dominant(px(48, 48), 0), "the centre: {:?}", px(48, 48));
    assert!(dominant(px(48, 30), 0), "inside the top: {:?}", px(48, 30));
    assert!(
        !dominant(px(28, 28), 0),
        "the sign's corner: {:?}",
        px(28, 28)
    );
}
