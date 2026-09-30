//! Tests for world-canvas interaction (#429): the camera ray through the pointer
//! hits a `WorldSpace` canvas — through the screen centre while the cursor is
//! locked — screen canvases still win, and a wall in front stops the ray.

use glam::{Vec2, Vec3};

use super::fixture::{panel, scene, Handlers, Rig, SCREEN};
use super::UiHook::*;
use crate::components::{CanvasComponent, CanvasRenderMode};
use crate::scene::Camera;

/// A 400×200 `WorldSpace` canvas (4×2 m) at the origin with a clickable left half,
/// seen by a camera 5 m in front of it, 1 m left of its centre. Returns the rig
/// and the panel.
fn terminal() -> (Rig, u32) {
    let (mut s, canvas) = scene();
    let c = CanvasComponent {
        render_mode: CanvasRenderMode::WorldSpace,
        reference_resolution: Vec2::new(400.0, 200.0),
        ..Default::default()
    };
    s.world.set_canvas(canvas, Some(c));
    let left = panel(&mut s, canvas, Vec2::ZERO, Vec2::new(200.0, 200.0));
    let h = Handlers::default().on(left, &[PointerEnter, PointerClick]);
    let mut rig = Rig::new(s, h);
    rig.camera = Some(Camera::new(Vec3::new(-1.0, 0.0, 5.0), -90.0, 0.0));
    (rig, left)
}

fn click(rig: &mut Rig) -> Vec<(u32, super::UiHook)> {
    rig.input.press("Mouse0");
    rig.input.release("Mouse0");
    rig.hooks()
}

#[test]
fn a_locked_cursor_clicks_what_the_screen_centre_looks_at() {
    let (mut rig, left) = terminal();
    rig.input.set_cursor_locked(true);
    assert_eq!(
        click(&mut rig),
        vec![(left, PointerEnter), (left, PointerClick)]
    );
    assert!(rig.events.is_pointer_over_ui());
    // Turn to look right of the canvas centre: the right half has no graphic.
    rig.camera = Some(Camera::new(Vec3::new(1.0, 0.0, 5.0), -90.0, 0.0));
    assert!(click(&mut rig).is_empty());
    assert!(!rig.events.is_pointer_over_ui());
}

#[test]
fn a_free_pointer_casts_its_own_ray_and_screen_canvases_win() {
    let (mut rig, left) = terminal();
    // The screen centre looks at the left half; the right edge of the screen does not.
    rig.point(SCREEN.x - 1.0, SCREEN.y * 0.5);
    assert!(rig.hooks().is_empty());
    rig.point(SCREEN.x * 0.5, SCREEN.y * 0.5);
    assert_eq!(rig.hooks(), vec![(left, PointerEnter)]);
    // An overlay panel over the same point is on top.
    let overlay = rig.scene.add_entity("Hud".to_string());
    let hud = Some(CanvasComponent::default());
    rig.scene.world.set_canvas(overlay, hud);
    let cover = panel(&mut rig.scene, overlay, Vec2::ZERO, SCREEN);
    rig.handlers.0.insert((cover, PointerEnter));
    rig.point(SCREEN.x * 0.5 + 1.0, SCREEN.y * 0.5);
    assert_eq!(rig.hooks(), vec![(cover, PointerEnter)]);
}

#[test]
fn a_wall_in_front_stops_the_ray() {
    let (mut rig, _) = terminal();
    rig.input.set_cursor_locked(true);
    rig.wall = 3.0;
    assert!(
        click(&mut rig).is_empty(),
        "the canvas is 5 m away, the wall 3 m"
    );
    rig.wall = 5.5;
    assert_eq!(click(&mut rig).len(), 2, "a wall behind it does not matter");
}
