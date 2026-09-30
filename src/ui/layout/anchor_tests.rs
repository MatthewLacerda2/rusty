use std::f32::consts::FRAC_PI_2;

use glam::{Vec2, Vec3};

use super::*;
use crate::components::WorldAnchor;
use crate::scene::{Camera, Scene};

const SIZE: Vec2 = Vec2::new(1920.0, 1080.0);

fn anchor(clamp: bool, rotate: bool, hide: bool) -> WorldAnchor {
    WorldAnchor {
        clamp_to_screen_edge: clamp,
        edge_padding: 20.0,
        rotate_toward_target: rotate,
        hide_when_behind: hide,
        ..Default::default()
    }
}

fn at(p: Placement) -> (Vec2, f32) {
    match p {
        Placement::At { pivot, angle } => (pivot, angle),
        Placement::Hidden => panic!("expected a placement"),
    }
}

#[test]
fn on_screen_markers_sit_at_the_projection_upright() {
    let a = anchor(true, true, true);
    let (p, angle) = at(clamp(&a, Vec2::new(300.0, 400.0), false, SIZE));
    assert_eq!((p, angle), (Vec2::new(300.0, 400.0), 0.0));
}

#[test]
fn off_screen_markers_clamp_to_the_padded_edge_toward_the_target() {
    let a = anchor(true, true, true);
    // Far right, level with the centre: the right edge, pointing right.
    let (p, angle) = at(clamp(&a, Vec2::new(5000.0, 540.0), false, SIZE));
    assert!((p - Vec2::new(1900.0, 540.0)).length() < 1e-3, "{p}");
    assert!((angle + FRAC_PI_2).abs() < 1e-5, "up turned to +X is -90°");
    // Up and to the left: whichever edge the ray from the centre reaches first.
    let (p, _) = at(clamp(&a, Vec2::new(-960.0, 3240.0), false, SIZE));
    assert!(
        (p.y - 1060.0).abs() < 1e-3 && p.x > 20.0 && p.x < 960.0,
        "{p}"
    );
    // Unclamped: the raw projection.
    let raw = anchor(false, false, true);
    let (p, _) = at(clamp(&raw, Vec2::new(5000.0, 540.0), false, SIZE));
    assert_eq!(p, Vec2::new(5000.0, 540.0));
}

#[test]
fn behind_the_camera_hides_or_flips_to_the_edge() {
    // Mirrored left of centre means the target is behind and to the right.
    let mirrored = Vec2::new(700.0, 540.0);
    assert_eq!(
        clamp(&anchor(true, true, true), mirrored, true, SIZE),
        Placement::Hidden
    );
    let (p, _) = at(clamp(&anchor(true, false, false), mirrored, true, SIZE));
    assert!((p - Vec2::new(1900.0, 540.0)).length() < 1e-3, "{p}");
    // Dead behind: straight down.
    let (p, angle) = at(clamp(&anchor(true, true, false), SIZE * 0.5, true, SIZE));
    assert!((p - Vec2::new(960.0, 20.0)).length() < 1e-3, "{p}");
    assert!((angle.abs() - std::f32::consts::PI).abs() < 1e-5);
}

#[test]
fn place_projects_the_target_through_the_camera() {
    let mut scene = Scene::new();
    let enemy = scene.add_entity("Enemy".to_string());
    if let Some(mut t) = scene.world.transform_mut(enemy) {
        t.position = Vec3::new(0.0, 0.0, -10.0);
    }
    let cam = Camera::new(Vec3::ZERO, -90.0, 0.0);
    let view = UiView::with_camera(SIZE, cam);
    let a = WorldAnchor {
        target: Some(enemy),
        offset: Vec3::ZERO,
        ..Default::default()
    };
    let (p, _) = at(place(&scene.world, &a, &view, SIZE, 1.0).expect("a camera"));
    assert!((p - SIZE * 0.5).length() < 0.5, "{p}");
    // No camera: the anchors place it.
    assert_eq!(
        place(&scene.world, &a, &UiView::screen(SIZE), SIZE, 1.0),
        None
    );
    // A destroyed target hides the marker.
    scene.world.despawn(enemy);
    assert_eq!(
        place(&scene.world, &a, &view, SIZE, 1.0),
        Some(Placement::Hidden)
    );
}

#[test]
fn the_layout_pins_markers_and_drops_hidden_ones_with_their_subtree() {
    use crate::components::{CanvasComponent, RectTransformComponent};
    use crate::ui::layout::{rect_in, UiLayout};
    let mut scene = Scene::new();
    let canvas = scene.add_entity("Hud".to_string());
    scene
        .world
        .set_canvas(canvas, Some(CanvasComponent::default()));
    let marker = scene.add_entity("Marker".to_string());
    let rt = RectTransformComponent {
        world_anchor: Some(WorldAnchor {
            offset: Vec3::new(0.0, 0.0, -10.0),
            ..Default::default()
        }),
        ..Default::default()
    };
    scene.world.set_rect_transform(marker, Some(rt));
    scene.set_parent(marker, Some(canvas)).expect("parent");
    let label = scene.add_entity("Label".to_string());
    scene
        .world
        .set_rect_transform(label, Some(RectTransformComponent::default()));
    scene.set_parent(label, Some(marker)).expect("parent");
    // Looking at the point: the 100×100 marker is centred on screen.
    let ahead = UiView::with_camera(SIZE, Camera::new(Vec3::ZERO, -90.0, 0.0));
    let layout = UiLayout::compute_in(&scene.world, &ahead);
    let r = layout.get(marker).expect("placed");
    assert!(
        (r.rect.0 - Vec2::new(910.0, 490.0)).length() < 0.5,
        "{:?}",
        r.rect
    );
    assert_eq!(rect_in(&scene.world, marker, &ahead), Some(*r));
    assert!(layout.get(label).is_some());
    // Turned away: hidden, and its child with it.
    let away = UiView::with_camera(SIZE, Camera::new(Vec3::ZERO, 90.0, 0.0));
    let layout = UiLayout::compute_in(&scene.world, &away);
    assert!(layout.get(marker).is_none() && layout.get(label).is_none());
    assert_eq!(rect_in(&scene.world, marker, &away), None);
}
