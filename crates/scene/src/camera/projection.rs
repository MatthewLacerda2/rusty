use glam::camera::rh::view::look_at_mat4;
use glam::{Mat4, Vec2, Vec3};

use super::Camera;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScreenPoint {
    pub position: Vec2,
    pub depth: f32,
}

impl ScreenPoint {
    pub fn is_behind(&self) -> bool {
        self.depth <= 0.0
    }

    pub fn is_on_screen(&self, screen: Vec2) -> bool {
        !self.is_behind()
            && self.position.cmpge(Vec2::ZERO).all()
            && self.position.cmple(screen).all()
    }
}

impl Camera {
    pub fn view_matrix(&self) -> Mat4 {
        look_at_mat4(self.position, self.position + self.forward(), self.up)
    }

    pub fn to_world(&self) -> Mat4 {
        self.view_matrix().inverse()
    }

    pub fn world_to_screen(&self, p: Vec3, screen: Vec2) -> ScreenPoint {
        let screen = screen.max(Vec2::ONE);
        let clip = self.build_view_projection(screen.x / screen.y) * p.extend(1.0);
        let w = if clip.w.abs() < 1e-6 { 1e-6 } else { clip.w };
        let ndc = clip.truncate().truncate() / w;
        ScreenPoint {
            position: (ndc * 0.5 + 0.5) * screen,
            // Not clip `w`: an orthographic camera's is always 1.
            depth: (p - self.position).dot(self.forward()),
        }
    }

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
