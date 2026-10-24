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
//! | Alt + RMB drag, mouse wheel, or pinch | zoom toward the pivot |
//! | two-finger trackpad scroll | look (Shift: pan), [`scroll`] (#1344) |
//! | arrow keys (viewport hovered) | look |
//! | F (viewport hovered) | frame the selection |
//!
//! WASD/QE fly only while RMB is held, as in Unity, which keeps them free for tool
//! hotkeys. [`BINDINGS`] is this table as the viewport's help hint shows it.
//!
//! A gesture only starts with the button pressed on the viewport, so a drag that
//! began on a panel never turns the camera. Plain LMB stays the gizmo/pick/rect tool.

use glam::{Vec2, Vec3};

use super::scene_camera::{SceneCamera, POINTS_PER_STEP, ZOOM_PER_STEP};
use crate::scene::Scene;

pub mod scroll;

/// Points of look the arrow keys turn per second (90 degrees).
pub const ARROW_POINTS_PER_SECOND: f32 = 360.0;

/// The navigation bindings, (input, gesture), for the viewport's help hint.
pub const BINDINGS: &[(&str, &str)] = &[
    ("RMB drag", "look"),
    ("RMB + WASD / Q E", "fly (Shift faster, scroll = speed)"),
    ("Two-finger scroll", "look"),
    ("Shift + two-finger scroll", "pan"),
    ("Arrow keys", "look"),
    ("MMB drag", "pan"),
    ("Alt + LMB drag", "orbit"),
    ("Mouse wheel, pinch, Alt + RMB drag", "zoom"),
    ("F", "frame the selection"),
];

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
    /// Mouse-wheel scroll in points while hovered; positive is away from the user.
    pub scroll: f32,
    /// Two-finger trackpad scroll in points while hovered (look).
    pub swipe: Vec2,
    /// Shift + two-finger trackpad scroll in points while hovered (pan).
    pub swipe_pan: Vec2,
    /// Zoom steps from a pinch this frame, [`pinch_steps`].
    pub pinch: f32,
    /// Arrow-key look axes (right, down), each in -1..=1.
    pub turn: Vec2,
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
/// where it changes the fly speed (Unity's behaviour); a trackpad swipe looks or pans.
pub fn apply(camera: &mut SceneCamera, f: &NavFrame) {
    let steps = f.scroll / POINTS_PER_STEP;
    let turn = f.turn * ARROW_POINTS_PER_SECOND * f.dt;
    match f.gesture {
        Some(Gesture::Look) => {
            look(camera, f.motion + turn);
            camera.fly(f.fly, f.fast, f.dt);
            camera.adjust_fly_speed(steps + f.swipe.y / POINTS_PER_STEP);
            return;
        }
        Some(Gesture::Orbit) => camera.orbit(f.motion),
        Some(Gesture::Pan) => camera.pan(f.delta, f.viewport_height),
        // Dragging right or up zooms in.
        Some(Gesture::Zoom) => camera.zoom((f.motion.x - f.motion.y) / POINTS_PER_STEP),
        None => {}
    }
    camera.zoom(steps + f.pinch);
    look(camera, f.swipe + turn);
    if f.swipe_pan != Vec2::ZERO {
        camera.pan(f.swipe_pan, f.viewport_height);
    }
}

/// Look by `delta` points, skipping a still frame so the pivot never drifts.
fn look(camera: &mut SceneCamera, delta: Vec2) {
    if delta != Vec2::ZERO {
        camera.look(delta);
    }
}

/// The zoom steps a pinch of `factor` (egui's `zoom_delta`, > 1 spreads) is worth:
/// spreading to twice the size halves the distance to the pivot.
pub fn pinch_steps(factor: f32) -> f32 {
    if factor > 0.0 {
        factor.ln() / -(1.0 - ZOOM_PER_STEP).ln()
    } else {
        0.0
    }
}

/// The world-space sphere (center, radius) that frames entity `id`: the union of the
/// mesh bounds in its subtree, or a unit sphere at its position when it has none.
/// `None` when the entity is gone.
pub fn selection_bounds(scene: &Scene, id: u32) -> Option<(Vec3, f32)> {
    if !scene.world.contains(id) {
        return None;
    }
    let mut bounds: Option<(Vec3, Vec3)> = None;
    for next in scene.world.self_and_descendants(id) {
        let Some(mesh) = scene.world.mesh(next) else {
            continue;
        };
        if !scene.world.is_active(next) {
            continue;
        }
        if let Some((lo, hi)) = mesh.world_aabb(scene.world.local_to_world(next)) {
            bounds = Some(bounds.map_or((lo, hi), |(a, b)| (a.min(lo), b.max(hi))));
        }
    }
    Some(match bounds {
        Some((lo, hi)) => ((lo + hi) * 0.5, (hi - lo).length() * 0.5),
        None => (scene.world.world_position(id), 1.0),
    })
}

/// Read this frame's navigation from the viewport image's `response`. `held` is
/// whether a button was pressed on the image and is still down.
pub fn read(response: &egui::Response, held: bool) -> NavFrame {
    let viewport_height = response.rect.height();
    let line_points = response.ctx.options(|o| o.input_options.line_scroll_speed);
    let hovered = response.hovered() || held;
    let keys = hovered && !response.ctx.text_edit_focused();
    response.ctx.input(|i| {
        let buttons = Buttons {
            primary: i.pointer.primary_down(),
            secondary: i.pointer.secondary_down(),
            middle: i.pointer.middle_down(),
            alt: i.modifiers.alt,
        };
        let gesture = held.then(|| gesture(buttons)).flatten();
        let axis = |pos, neg| axis(i, pos, neg);
        use egui::Key;
        let wheel = if hovered {
            scroll::split(&i.events, line_points, viewport_height)
        } else {
            scroll::Scroll::default()
        };
        let arrows = Vec2::new(
            axis(Key::ArrowRight, Key::ArrowLeft),
            axis(Key::ArrowDown, Key::ArrowUp),
        );
        NavFrame {
            gesture,
            motion: to_vec2(i.pointer.motion().unwrap_or_else(|| i.pointer.delta())),
            delta: to_vec2(i.pointer.delta()),
            scroll: wheel.wheel,
            swipe: wheel.swipe,
            swipe_pan: wheel.swipe_pan,
            pinch: if hovered {
                pinch_steps(i.zoom_delta())
            } else {
                0.0
            },
            turn: if keys { arrows } else { Vec2::ZERO },
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

/// One key axis: 1 with `pos` held, -1 with `neg`, 0 with both or neither.
fn axis(i: &egui::InputState, pos: egui::Key, neg: egui::Key) -> f32 {
    f32::from(u8::from(i.key_down(pos))) - f32::from(u8::from(i.key_down(neg)))
}

fn to_vec2(v: egui::Vec2) -> Vec2 {
    Vec2::new(v.x, v.y)
}

#[cfg(test)]
#[path = "scene_nav_tests.rs"]
mod scene_nav_tests;

#[cfg(test)]
#[path = "scene_nav/trackpad_tests.rs"]
mod trackpad_tests;
