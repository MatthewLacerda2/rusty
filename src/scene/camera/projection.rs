//! src/scene/camera/projection.rs — world ↔ screen through the active camera (#429).
//!
//! Pure glam maths over the sim's [`Camera`], so projection runs in the sim (and
//! headless) exactly as it draws: `Camera.WorldToScreen` / `ScreenToWorldRay`, the
//! UI's world-anchored markers and its world-canvas hit-testing all go through here.
//!
//! **Screen frame.** UI screen pixels — bottom-left origin, y-up — the frame
//! `UI.GetRect(id).screen` and the UI pointer are in (Unity's screen space). The
//! screen is a sim input (`ui::ScreenSize`), so a projection is a pure function of
//! (camera, screen size).

use glam::{Mat4, Vec2, Vec3};

use super::Camera;

/// A world point seen through a camera.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScreenPoint {
    /// Where it lands, in UI screen pixels (bottom-left origin, y-up). A point
    /// behind the camera projects mirrored through the centre, as in Unity.
    pub position: Vec2,
    /// Its distance in front of the camera along the view direction; `<= 0` is
    /// behind the camera.
    pub depth: f32,
}

impl ScreenPoint {
    /// Whether the point is behind the camera (or on its plane).
    pub fn is_behind(&self) -> bool {
        self.depth <= 0.0
    }

    /// Whether the point is in front of the camera and inside a `screen`-pixel screen.
    pub fn is_on_screen(&self, screen: Vec2) -> bool {
        !self.is_behind()
            && self.position.cmpge(Vec2::ZERO).all()
            && self.position.cmple(screen).all()
    }
}

impl Camera {
    /// The world → view matrix (the view looks down its local -Z, y up).
    pub fn view_matrix(&self) -> Mat4 {
        Mat4::look_at_rh(self.position, self.position + self.forward(), Vec3::Y)
    }

    /// The camera's own frame → world: local +X is right, +Y up, -Z forward.
    pub fn to_world(&self) -> Mat4 {
        self.view_matrix().inverse()
    }

    /// Where world point `p` lands on a `screen`-pixel screen (see [`ScreenPoint`]).
    pub fn world_to_screen(&self, p: Vec3, screen: Vec2) -> ScreenPoint {
        let screen = screen.max(Vec2::ONE);
        let clip = self.build_view_projection(screen.x / screen.y) * p.extend(1.0);
        // A perspective clip `w` is the view-space distance along the forward axis.
        let w = if clip.w.abs() < 1e-6 { 1e-6 } else { clip.w };
        let ndc = clip.truncate().truncate() / w;
        ScreenPoint {
            position: (ndc * 0.5 + 0.5) * screen,
            // Distance along the view axis: equals a perspective clip `w`, and stays
            // right for an orthographic camera, whose `w` is always 1 (#430).
            depth: (p - self.position).dot(self.forward()),
        }
    }

    /// The world ray through screen pixel `px` (bottom-left origin, y-up) on a
    /// `screen`-pixel screen: `(origin, unit direction)`. The origin is on the near
    /// plane, so nothing between the eye and the near plane is ever hit.
    pub fn screen_ray(&self, px: Vec2, screen: Vec2) -> (Vec3, Vec3) {
        let screen = screen.max(Vec2::ONE);
        let inv = self.build_view_projection(screen.x / screen.y).inverse();
        let ndc = px / screen * 2.0 - Vec2::ONE;
        let near = inv.project_point3(ndc.extend(0.0));
        let far = inv.project_point3(ndc.extend(1.0));
        let dir = (far - near)
            .try_normalize()
            .unwrap_or_else(|| self.forward());
        (near, dir)
    }
}

#[cfg(test)]
#[path = "projection_tests.rs"]
mod projection_tests;
