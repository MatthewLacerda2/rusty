//! The rect tool end to end in viewport points: the overlay mapping, what is
//! editable, a drag written through the shared setters, and click-select.

use glam::{Vec2, Vec3};

use super::*;
use crate::components::{CanvasComponent, ImageComponent, LayoutGroupComponent};
use crate::scene::Scene;

/// A 960×540-point viewport at 2 pixels per point: a 1920×1080 screen, where the
/// default canvas is 1:1 with its reference resolution.
const FRAME: OverlayFrame = OverlayFrame {
    size: Vec2::new(960.0, 540.0),
    pixels_per_point: 2.0,
};

/// A canvas holding a centred 200×100 image. Returns (scene, canvas, image).
fn hud() -> (Scene, u32, u32) {
    let mut s = Scene::new();
    let canvas = s.add_entity("Canvas".to_string());
    s.world.set_canvas(canvas, Some(CanvasComponent::default()));
    let img = s.add_entity("Image".to_string());
    let rt = RectTransformComponent {
        size_delta: Vec2::new(200.0, 100.0),
        ..RectTransformComponent::default()
    };
    s.world.set_rect_transform(img, Some(rt));
    s.world.set_image(img, Some(ImageComponent::default()));
    s.set_parent(img, Some(canvas)).expect("parent");
    (s, canvas, img)
}

#[test]
fn the_frame_maps_points_to_pixels_and_canvas_units() {
    assert_eq!(FRAME.screen(), Vec2::new(1920.0, 1080.0));
    assert_eq!(FRAME.pixel(Vec2::new(10.0, 40.0)), Vec2::new(20.0, 1000.0));
    assert_eq!(
        FRAME.to_local(Vec2::new(20.0, 1000.0), 1.0),
        Vec2::new(10.0, 40.0)
    );
    assert_eq!(
        FRAME.delta_to_canvas(Vec2::new(3.0, 4.0), 2.0),
        Vec2::new(3.0, -4.0)
    );
}

#[test]
fn roots_markers_and_driven_children_are_read_only() {
    let (mut s, canvas, img) = hud();
    let layout = FRAME.layout(&s.world);
    assert!(
        target(&s.world, &layout, img)
            .expect("on the overlay")
            .editable
    );
    assert!(
        !target(&s.world, &layout, canvas)
            .expect("the root")
            .editable
    );
    s.world.rect_transform_mut(img).expect("rt").world_anchor = Some(Default::default());
    assert!(
        !target(&s.world, &FRAME.layout(&s.world), img)
            .expect("a marker")
            .editable
    );
    s.world.rect_transform_mut(img).expect("rt").world_anchor = None;
    s.world
        .set_layout_group(canvas, Some(LayoutGroupComponent::default()));
    assert!(
        !target(&s.world, &FRAME.layout(&s.world), img)
            .expect("driven")
            .editable
    );
    assert!(begin(&s.world, &FRAME, img, Vec2::new(480.0, 270.0)).is_none());
}

#[test]
fn a_body_drag_in_points_moves_the_anchored_position() {
    let (mut s, _, img) = hud();
    // The image covers points 430..530 × 245..295; grab its middle-left.
    let drag = begin(&s.world, &FRAME, img, Vec2::new(450.0, 270.0)).expect("grabbed");
    assert_eq!(drag.handle, Handle::Body);
    assert!(apply(&mut s.world, &FRAME, drag, Vec2::new(5.0, 10.0)));
    let rt = s.world.rect_transform(img).expect("rt").clone();
    assert_eq!(rt.anchored_position, Vec2::new(10.0, -20.0));
    // The right edge, dragged 10 points right: 20 units wider, centre pivot follows.
    let edge = begin(&s.world, &FRAME, img, Vec2::new(535.0, 280.0)).expect("grabbed");
    assert_eq!(edge.handle, Handle::Resize { x: 1, y: 0 });
    apply(&mut s.world, &FRAME, edge, Vec2::new(10.0, 0.0));
    let rt = s.world.rect_transform(img).expect("rt").clone();
    assert_eq!(rt.size_delta, Vec2::new(220.0, 100.0));
    assert_eq!(rt.anchored_position, Vec2::new(20.0, -20.0));
}

#[test]
fn a_click_picks_the_overlay_only_while_it_shows() {
    let (s, _, img) = hud();
    let away = (Vec3::new(0.0, 0.0, 5.0), Vec3::Z);
    let p = Vec2::new(480.0, 270.0);
    assert_eq!(
        pick(&s.world, &FRAME, p, true, away, f32::INFINITY),
        Some(img)
    );
    assert_eq!(pick(&s.world, &FRAME, p, false, away, f32::INFINITY), None);
    assert_eq!(
        pick(&s.world, &FRAME, Vec2::ZERO, true, away, f32::INFINITY),
        None
    );
}
