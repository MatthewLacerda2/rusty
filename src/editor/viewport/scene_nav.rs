//! src/editor/viewport/scene_nav.rs — Unity's Scene view controls, read from egui (#745).
//!
//! The Scene camera is driven from the viewport image's egui response, never from the
//! game's `InputState` (which the shell gates to Play + a focused Game view, #416).
//! Unity's bindings:
//!
//! | input | gesture |
//! |---|---|
//! | RMB held | look; with WASD fly, Q/E down/up, Shift faster, scroll = fly speed |
//! | MMB drag | pan |
//! | Alt + LMB drag | orbit around the pivot |
//! | Alt + RMB drag, or scroll | zoom toward the pivot |
//! | F (viewport hovered) | frame the selection |
//!
//! A gesture only starts with the button pressed on the viewport, so a drag that
//! began on a panel never turns the camera. Plain LMB stays the gizmo/pick/rect tool.

use glam::{Vec2, Vec3};

use super::scene_camera::{SceneCamera, POINTS_PER_STEP};
use crate::scene::Scene;

/// The Scene view's navigation state, kept on the editor across frames.
#[derive(Clone, Copy, Debug, Default)]
pub struct SceneView {
    /// The pose the Scene tab renders from.
    pub camera: SceneCamera,
    /// RMB mouse look is held: the shell hides and locks the cursor.
    pub looking: bool,
    /// A camera gesture is held or Alt is down: LMB belongs to the camera, not the tools.
    pub navigating: bool,
}

/// What the held buttons mean this frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gesture {
    Look,
    Orbit,
    Pan,
    Zoom,
}

/// Which buttons are held (pressed on the viewport) and whether Alt is down.
#[derive(Clone, Copy, Debug, Default)]
pub struct Buttons {
    pub primary: bool,
    pub secondary: bool,
    pub middle: bool,
    pub alt: bool,
}

/// Unity's mapping of buttons to a gesture; `None` leaves the pointer to the tools.
pub fn gesture(b: Buttons) -> Option<Gesture> {
    if b.middle {
        Some(Gesture::Pan)
    } else if b.alt && b.primary {
        Some(Gesture::Orbit)
    } else if b.alt && b.secondary {
        Some(Gesture::Zoom)
    } else if b.secondary {
        Some(Gesture::Look)
    } else {
        None
    }
}

/// One frame of Scene-view navigation input.
#[derive(Clone, Copy, Debug, Default)]
pub struct NavFrame {
    pub gesture: Option<Gesture>,
    /// Pointer motion this frame in points (raw mouse motion when the OS sends it,
    /// so a locked cursor still looks around).
    pub motion: Vec2,
    /// Pointer movement in points, for pan to track the cursor exactly.
    pub delta: Vec2,
    /// Vertical scroll in points while hovered; positive is away from the user.
    pub scroll: f32,
    /// Fly axes (right, up, forward) from WASD/QE, each in -1..=1.
    pub fly: Vec3,
    /// Shift held.
    pub fast: bool,
    /// The viewport's height in points.
    pub viewport_height: f32,
    /// Seconds since the last frame.
    pub dt: f32,
}

/// Apply one frame of navigation to the camera. Scroll zooms, except while looking,
/// where it changes the fly speed (Unity's behaviour).
pub fn apply(camera: &mut SceneCamera, f: &NavFrame) {
    let steps = f.scroll / POINTS_PER_STEP;
    match f.gesture {
        Some(Gesture::Look) => {
            camera.look(f.motion);
            camera.fly(f.fly, f.fast, f.dt);
            camera.adjust_fly_speed(steps);
            return;
        }
        Some(Gesture::Orbit) => camera.orbit(f.motion),
        Some(Gesture::Pan) => camera.pan(f.delta, f.viewport_height),
        // Dragging right or up zooms in.
        Some(Gesture::Zoom) => camera.zoom((f.motion.x - f.motion.y) / POINTS_PER_STEP),
        None => {}
    }
    camera.zoom(steps);
}

/// The world-space sphere (center, radius) that frames entity `id`: the union of the
/// mesh bounds in its subtree, or a unit sphere at its position when it has none.
/// `None` when the entity is gone.
pub fn selection_bounds(scene: &Scene, id: u32) -> Option<(Vec3, f32)> {
    if !scene.world.contains(id) {
        return None;
    }
    let mut bounds: Option<(Vec3, Vec3)> = None;
    let mut stack = vec![id];
    while let Some(next) = stack.pop() {
        stack.extend(scene.world.children(next));
        let Some(mesh) = scene.world.mesh(next) else {
            continue;
        };
        if !scene.world.is_active(next) {
            continue;
        }
        if let Some((lo, hi)) = mesh.world_aabb(scene.compute_world_matrix(next)) {
            bounds = Some(bounds.map_or((lo, hi), |(a, b)| (a.min(lo), b.max(hi))));
        }
    }
    Some(match bounds {
        Some((lo, hi)) => ((lo + hi) * 0.5, (hi - lo).length() * 0.5),
        None => (scene.compute_world_matrix(id).col(3).truncate(), 1.0),
    })
}

/// Read this frame's navigation from the viewport image's `response`. `held` is
/// whether a button was pressed on the image and is still down.
pub fn read(response: &egui::Response, held: bool) -> NavFrame {
    let viewport_height = response.rect.height();
    response.ctx.input(|i| {
        let buttons = Buttons {
            primary: i.pointer.primary_down(),
            secondary: i.pointer.secondary_down(),
            middle: i.pointer.middle_down(),
            alt: i.modifiers.alt,
        };
        let gesture = held.then(|| gesture(buttons)).flatten();
        let axis = |pos: egui::Key, neg: egui::Key| {
            f32::from(u8::from(i.key_down(pos))) - f32::from(u8::from(i.key_down(neg)))
        };
        use egui::Key;
        NavFrame {
            gesture,
            motion: to_vec2(i.pointer.motion().unwrap_or_else(|| i.pointer.delta())),
            delta: to_vec2(i.pointer.delta()),
            scroll: if response.hovered() || held {
                i.smooth_scroll_delta.y
            } else {
                0.0
            },
            fly: Vec3::new(
                axis(Key::D, Key::A),
                axis(Key::E, Key::Q),
                axis(Key::W, Key::S),
            ),
            fast: i.modifiers.shift,
            viewport_height,
            dt: i.stable_dt,
        }
    })
}

fn to_vec2(v: egui::Vec2) -> Vec2 {
    Vec2::new(v.x, v.y)
}

#[cfg(test)]
#[path = "scene_nav_tests.rs"]
mod scene_nav_tests;
