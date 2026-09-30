//! Tests for the UI layout pass (#417): anchor-point vs anchor-stretch, pivot,
//! nested rects, the scaler's match modes, rotation/scale around the pivot, draw
//! order, and that the on-demand [`rect_of`] agrees with the full pass.

use glam::{Quat, Vec2, Vec3};

use super::*;
use crate::components::{CanvasComponent, RectTransformComponent};
use crate::scene::Scene;

const HD: Vec2 = Vec2::new(1920.0, 1080.0);

fn assert_near(a: Vec2, b: Vec2) {
    assert!((a - b).abs().max_element() < 1e-3, "{a} != {b}");
}

fn canvas(scene: &mut Scene, sort_order: i32) -> u32 {
    let id = scene.add_entity("Canvas".to_string());
    let c = CanvasComponent {
        sort_order,
        ..CanvasComponent::default()
    };
    scene.world.set_canvas(id, Some(c));
    id
}

fn element(scene: &mut Scene, parent: u32, rt: RectTransformComponent) -> u32 {
    let id = scene.add_entity("Element".to_string());
    scene.world.set_rect_transform(id, Some(rt));
    scene.set_parent(id, Some(parent)).expect("parent exists");
    id
}

fn point(anchor: Vec2, pivot: Vec2, pos: Vec2, size: Vec2) -> RectTransformComponent {
    RectTransformComponent {
        anchor_min: anchor,
        anchor_max: anchor,
        pivot,
        anchored_position: pos,
        size_delta: size,
    }
}

#[test]
fn anchor_point_places_the_pivot_off_the_anchor() {
    let mut scene = Scene::new();
    let root = canvas(&mut scene, 0);
    // Top-right corner anchor, pivot at the element's top-right, pulled in by 10.
    let rt = point(
        Vec2::ONE,
        Vec2::ONE,
        Vec2::new(-10.0, -10.0),
        Vec2::new(200.0, 50.0),
    );
    let id = element(&mut scene, root, rt);
    let layout = UiLayout::compute(&scene.world, HD);
    let r = layout.get(id).expect("laid out");
    assert_near(r.rect.0, Vec2::new(1710.0, 1020.0));
    assert_near(r.rect.1, Vec2::new(200.0, 50.0));
}

#[test]
fn anchor_stretch_insets_from_the_parent_edges() {
    let mut scene = Scene::new();
    let root = canvas(&mut scene, 0);
    let rt = RectTransformComponent {
        anchor_min: Vec2::ZERO,
        anchor_max: Vec2::ONE,
        pivot: Vec2::splat(0.5),
        anchored_position: Vec2::ZERO,
        size_delta: Vec2::new(-40.0, -20.0),
    };
    let id = element(&mut scene, root, rt);
    let r = UiLayout::compute(&scene.world, HD).get(id).copied();
    let r = r.expect("laid out");
    assert_near(r.rect.0, Vec2::new(20.0, 10.0));
    assert_near(r.rect.1, Vec2::new(1880.0, 1060.0));
}

#[test]
fn nested_rects_lay_out_inside_their_parent() {
    let mut scene = Scene::new();
    let root = canvas(&mut scene, 0);
    let panel = point(
        Vec2::ZERO,
        Vec2::ZERO,
        Vec2::new(100.0, 100.0),
        Vec2::splat(400.0),
    );
    let panel = element(&mut scene, root, panel);
    let child = point(
        Vec2::splat(0.5),
        Vec2::splat(0.5),
        Vec2::ZERO,
        Vec2::splat(40.0),
    );
    let child = element(&mut scene, panel, child);
    let layout = UiLayout::compute(&scene.world, HD);
    let r = layout.get(child).expect("laid out");
    assert_near(r.rect.0, Vec2::new(280.0, 280.0));
    assert_eq!(r.canvas, root);
    assert_eq!(rect_of(&scene.world, child, HD).as_ref(), Some(r));
}

#[test]
fn scaler_match_modes_blend_in_log_space() {
    let mut c = CanvasComponent::default();
    // Twice the reference width, same height.
    let wide = Vec2::new(3840.0, 1080.0);
    assert!((c.scale_factor(wide) - 2.0).abs() < 1e-5);
    c.match_width_or_height = 1.0;
    assert!((c.scale_factor(wide) - 1.0).abs() < 1e-5);
    c.match_width_or_height = 0.5;
    assert!((c.scale_factor(wide) - 2f32.sqrt()).abs() < 1e-5);
    // Half-size screen, matching width: the canvas stays 1920 reference units wide.
    c.match_width_or_height = 0.0;
    assert_near(c.size(Vec2::new(960.0, 540.0)), HD);
}

#[test]
fn rotation_and_scale_apply_around_the_pivot_and_carry_children() {
    let mut scene = Scene::new();
    let root = canvas(&mut scene, 0);
    let panel = point(
        Vec2::ZERO,
        Vec2::ZERO,
        Vec2::new(100.0, 100.0),
        Vec2::splat(100.0),
    );
    let panel = element(&mut scene, root, panel);
    if let Some(mut t) = scene.world.transform_mut(panel) {
        t.rotation = Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
        t.scale = Vec3::new(2.0, 2.0, 1.0);
    }
    // A child filling the panel exactly.
    let fill = RectTransformComponent {
        anchor_min: Vec2::ZERO,
        anchor_max: Vec2::ONE,
        size_delta: Vec2::ZERO,
        ..RectTransformComponent::default()
    };
    let child = element(&mut scene, panel, fill);
    let layout = UiLayout::compute(&scene.world, HD);
    let r = layout.get(panel).expect("panel");
    // Pivot (0,0) sits at (100,100); 90° CCW then ×2: the quad spans x∈[-100,100].
    assert_near(r.corners[0], Vec2::new(100.0, 100.0));
    assert_near(r.corners[2], Vec2::new(-100.0, 300.0));
    let (lo, hi) = r.bounds();
    assert_near(lo, Vec2::new(-100.0, 100.0));
    assert_near(hi, Vec2::new(100.0, 300.0));
    assert_eq!(layout.get(child).map(|c| c.corners), Some(r.corners));
}

#[test]
fn draw_order_is_sort_order_then_hierarchy_preorder() {
    let mut scene = Scene::new();
    let top = canvas(&mut scene, 5);
    let bottom = canvas(&mut scene, -1);
    let a = element(&mut scene, top, RectTransformComponent::default());
    let a1 = element(&mut scene, a, RectTransformComponent::default());
    let b = element(&mut scene, top, RectTransformComponent::default());
    // No RectTransform: ends the subtree, as does everything below it.
    let plain = scene.add_entity("Plain".to_string());
    scene.set_parent(plain, Some(top)).expect("parent exists");
    let hidden = element(&mut scene, plain, RectTransformComponent::default());
    let layout = UiLayout::compute(&scene.world, HD);
    let order: Vec<u32> = layout.iter().map(|(id, _)| id).collect();
    assert_eq!(order, vec![bottom, top, a, a1, b]);
    assert!(layout.get(hidden).is_none());
    assert!(rect_of(&scene.world, hidden, HD).is_none());
}

#[test]
fn layout_is_deterministic_across_runs() {
    let build = || {
        let mut scene = Scene::new();
        let root = canvas(&mut scene, 0);
        let rt = point(
            Vec2::new(0.25, 0.75),
            Vec2::ZERO,
            Vec2::new(3.0, -7.0),
            Vec2::ONE,
        );
        let e = element(&mut scene, root, rt);
        element(&mut scene, e, RectTransformComponent::default());
        UiLayout::compute(&scene.world, Vec2::new(1366.0, 768.0))
    };
    assert_eq!(build(), build());
}
