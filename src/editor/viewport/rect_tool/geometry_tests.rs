//! The rect tool's maths against the real layout pass: under a rotated, scaled
//! parent and with the element itself rotated, every handle's drag lands where it
//! should once the scene is laid out again.

use glam::{Quat, Vec2, Vec3};

use super::*;
use crate::components::{CanvasComponent, RectTransformComponent};
use crate::scene::Scene;
use crate::ui::UiLayout;

const HD: Vec2 = Vec2::new(1920.0, 1080.0);

/// A canvas holding a rotated, scaled panel holding a rotated 200×100 element with
/// an off-centre pivot. Returns the scene and the element.
fn scene() -> (Scene, u32) {
    let mut s = Scene::new();
    let canvas = s.add_entity("Canvas".to_string());
    s.world.set_canvas(canvas, Some(CanvasComponent::default()));
    let rt = |size: Vec2, pivot: Vec2| RectTransformComponent {
        size_delta: size,
        pivot,
        anchored_position: Vec2::new(30.0, -20.0),
        ..RectTransformComponent::default()
    };
    let panel = s.add_entity("Panel".to_string());
    s.world
        .set_rect_transform(panel, Some(rt(Vec2::splat(600.0), Vec2::splat(0.5))));
    s.set_parent(panel, Some(canvas)).expect("parent");
    let el = s.add_entity("El".to_string());
    s.world
        .set_rect_transform(el, Some(rt(Vec2::new(200.0, 100.0), Vec2::new(0.25, 0.7))));
    s.set_parent(el, Some(panel)).expect("parent");
    let mut t = s.world.transform_mut(panel).expect("transform");
    (t.rotation, t.scale) = (Quat::from_rotation_z(0.35), Vec3::new(1.5, 0.8, 1.0));
    drop(t);
    s.world.transform_mut(el).expect("transform").rotation = Quat::from_rotation_z(-0.6);
    (s, el)
}

fn rects(s: &Scene, id: u32) -> (UiRect, UiRect) {
    let layout = UiLayout::compute(&s.world, HD);
    let parent = s.world.parent_id(id).expect("parent");
    (
        *layout.get(id).expect("laid out"),
        *layout.get(parent).expect("laid out"),
    )
}

/// Drag `handle` by `delta` (canvas units) and return the corners before and after.
fn drag_and_relayout(handle: Handle, delta: Vec2) -> ([Vec2; 4], [Vec2; 4], (Scene, u32)) {
    let (mut s, el) = scene();
    let (own, parent) = rects(&s, el);
    let rt = s.world.rect_transform(el).expect("rt").clone();
    let edit = drag(&rt, handle, delta, &own, &parent).expect("a frame");
    let mut r = s.world.rect_transform_mut(el).expect("rt");
    (r.anchored_position, r.size_delta, r.pivot) =
        (edit.anchored_position, edit.size_delta, edit.pivot);
    drop(r);
    let after = rects(&s, el).0.corners;
    (own.corners, after, (s, el))
}

fn near(a: Vec2, b: Vec2) {
    assert!((a - b).abs().max_element() < 1e-2, "{a} != {b}");
}

#[test]
fn the_frame_maps_the_layout_rect_onto_its_quad() {
    let (s, el) = scene();
    let (own, _) = rects(&s, el);
    let f = frame_of(&own).expect("a frame");
    let (min, size) = own.rect;
    near(f.transform_point2(min), own.corners[0]);
    near(f.transform_point2(min + size), own.corners[2]);
}

#[test]
fn the_body_moves_the_whole_quad_by_the_drag() {
    let d = Vec2::new(40.0, -15.0);
    let (before, after, _) = drag_and_relayout(Handle::Body, d);
    for i in 0..4 {
        near(after[i], before[i] + d);
    }
}

#[test]
fn a_resize_holds_the_opposite_side() {
    let d = Vec2::new(25.0, 35.0);
    // Right edge: the left edge (bottom-left, top-left) stays.
    let (before, after, _) = drag_and_relayout(Handle::Resize { x: 1, y: 0 }, d);
    near(after[0], before[0]);
    near(after[1], before[1]);
    // Top-right corner: bottom-left stays; bottom-left corner: top-right stays.
    let (before, after, _) = drag_and_relayout(Handle::Resize { x: 1, y: 1 }, d);
    near(after[0], before[0]);
    let (before, after, _) = drag_and_relayout(Handle::Resize { x: -1, y: -1 }, d);
    near(after[2], before[2]);
    // The dragged corner follows the pointer.
    near(after[0], before[0] + d);
}

#[test]
fn the_pivot_moves_and_the_rect_stays() {
    let d = Vec2::new(-12.0, 9.0);
    let (s, el) = scene();
    let (own, _) = rects(&s, el);
    let pivot = s.world.rect_transform(el).expect("rt").pivot;
    let was = frame_of(&own)
        .expect("frame")
        .transform_point2(own.rect.0 + own.rect.1 * pivot);
    let (before, after, (s, el)) = drag_and_relayout(Handle::Pivot, d);
    for i in 0..4 {
        near(after[i], before[i]);
    }
    let (own, _) = rects(&s, el);
    let pivot = s.world.rect_transform(el).expect("rt").pivot;
    let now = frame_of(&own)
        .expect("frame")
        .transform_point2(own.rect.0 + own.rect.1 * pivot);
    near(now, was + d);
}

#[test]
fn hits_prefer_the_pivot_then_corners_then_the_body() {
    let r = UiRect {
        canvas: 0,
        scale_factor: 1.0,
        rect: (Vec2::ZERO, Vec2::new(100.0, 50.0)),
        corners: [
            Vec2::ZERO,
            Vec2::new(0.0, 50.0),
            Vec2::new(100.0, 50.0),
            Vec2::new(100.0, 0.0),
        ],
    };
    let id = |c: Vec2| c;
    let pivot = Vec2::splat(0.5);
    assert_eq!(
        hit(&r, pivot, id, Vec2::new(51.0, 24.0), 5.0),
        Some(Handle::Pivot)
    );
    assert_eq!(
        hit(&r, pivot, id, Vec2::new(98.0, 49.0), 5.0),
        Some(Handle::Resize { x: 1, y: 1 })
    );
    assert_eq!(
        hit(&r, pivot, id, Vec2::new(-2.0, 25.0), 5.0),
        Some(Handle::Resize { x: -1, y: 0 })
    );
    assert_eq!(
        hit(&r, pivot, id, Vec2::new(20.0, 10.0), 5.0),
        Some(Handle::Body)
    );
    assert_eq!(hit(&r, pivot, id, Vec2::new(120.0, 10.0), 5.0), None);
    let flat = UiRect {
        rect: (Vec2::ZERO, Vec2::new(100.0, 0.0)),
        ..r
    };
    assert!(frame_of(&flat).is_none() && handles(&flat, pivot).is_empty());
}
