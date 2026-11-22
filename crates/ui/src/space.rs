//! crates/ui/src/space.rs — where a canvas lives: on the screen, or on a plane in the world.

use std::rc::Rc;

use glam::{Mat4, Quat, Vec2, Vec3};

use crate::{TextureSizes, UiRect};
use rusty_components::{CanvasComponent, CanvasRenderMode};
use rusty_ecs::World;
use rusty_scene::Camera;

#[derive(Clone)]
pub struct UiView {
    /// The screen size in pixels (see `ScreenSize`).
    pub screen: Vec2,
    /// The camera looking through it; `None` places no world UI and no marker.
    pub camera: Option<Camera>,
    /// The native size of each texture an `Image` measures by (`layout::native`):
    /// the platform's shared table (`ScreenSize::textures`).
    pub textures: Rc<TextureSizes>,
}

impl UiView {
    /// A view with no camera: overlay layout only, Images measured by `textures`.
    pub fn screen(screen: Vec2, textures: Rc<TextureSizes>) -> Self {
        Self {
            screen: screen.max(Vec2::ONE),
            camera: None,
            textures,
        }
    }

    /// A view through `camera`, Images measured by `textures`.
    pub fn with_camera(screen: Vec2, camera: Camera, textures: Rc<TextureSizes>) -> Self {
        Self {
            screen: screen.max(Vec2::ONE),
            camera: Some(camera),
            textures,
        }
    }

    pub fn pointer(&self, mouse: Option<Vec2>) -> UiPointer {
        let through = mouse.unwrap_or(self.screen * 0.5);
        UiPointer {
            screen: mouse,
            ray: self
                .camera
                .as_ref()
                .map(|c| c.screen_ray(through, self.screen)),
            ray_limit: f32::INFINITY,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UiPointer {
    pub screen: Option<Vec2>,
    pub ray: Option<(Vec3, Vec3)>,
    pub ray_limit: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum CanvasSpace {
    #[default]
    Screen,
    World(Mat4),
}

/// The space canvas entity `id` (carrying `canvas`) lays out into, seen through `view`.
pub fn canvas_space(
    world: &World,
    id: u32,
    canvas: &CanvasComponent,
    view: &UiView,
) -> CanvasSpace {
    let size = canvas.size(view.screen);
    let centre = Mat4::from_translation((-size * 0.5).extend(0.0));
    match (canvas.render_mode, &view.camera) {
        (CanvasRenderMode::WorldSpace, _) => {
            let units = Mat4::from_scale(Vec3::splat(1.0 / canvas.pixels_per_unit.max(1e-3)));
            CanvasSpace::World(world.local_to_world(id) * units * centre)
        }
        (CanvasRenderMode::ScreenSpaceCamera, Some(cam)) => {
            let d = canvas.plane_distance.max(0.01);
            let half_h = d * (cam.fov.to_radians() * 0.5).tan();
            let half = Vec2::new(half_h * view.screen.x / view.screen.y, half_h);
            let fill = Mat4::from_scale((half * 2.0 / size).extend(1.0));
            let lag = canvas.sway_state.offset * canvas.sway;
            let swing = Quat::from_rotation_y(lag.x.to_radians())
                * Quat::from_rotation_x(-lag.y.to_radians());
            let tilt = Quat::from_rotation_y(canvas.tilt.y.to_radians())
                * Quat::from_rotation_x(-canvas.tilt.x.to_radians());
            let plane = Mat4::from_quat(swing)
                * Mat4::from_translation(Vec3::new(0.0, 0.0, -d))
                * Mat4::from_quat(tilt);
            CanvasSpace::World(cam.to_world() * plane * fill * centre)
        }
        _ => CanvasSpace::Screen,
    }
}

pub fn ray_to_canvas(to_world: Mat4, (origin, dir): (Vec3, Vec3)) -> Option<(Vec2, f32)> {
    let local = to_world.inverse();
    let o = local.transform_point3(origin);
    let d = local.transform_vector3(dir);
    if d.z.abs() < 1e-9 {
        return None;
    }
    let s = -o.z / d.z;
    if s < 0.0 || !s.is_finite() {
        return None;
    }
    let point = (o + d * s).truncate();
    let hit = to_world.transform_point3(point.extend(0.0));
    Some((point, (hit - origin).length()))
}

pub fn canvas_to_screen(space: CanvasSpace, scale: f32, p: Vec2, view: &UiView) -> Option<Vec2> {
    match (space, &view.camera) {
        (CanvasSpace::Screen, _) => Some(p * scale),
        (CanvasSpace::World(m), Some(cam)) => {
            let sp = cam.world_to_screen(m.transform_point3(p.extend(0.0)), view.screen);
            (!sp.is_behind()).then_some(sp.position)
        }
        (CanvasSpace::World(_), None) => None,
    }
}

/// The space of canvas entity `id` through `view` — the screen for a non-canvas.
pub fn space_of(world: &World, id: u32, view: &UiView) -> CanvasSpace {
    world
        .canvas(id)
        .map(|c| canvas_space(world, id, &c, view))
        .unwrap_or_default()
}

pub fn rect_screen_bounds(
    space: CanvasSpace,
    rect: &UiRect,
    view: &UiView,
) -> Option<(Vec2, Vec2)> {
    let mut corners = rect.corners.iter();
    let first = canvas_to_screen(space, rect.scale_factor, *corners.next()?, view)?;
    corners.try_fold((first, first), |(lo, hi), &c| {
        let p = canvas_to_screen(space, rect.scale_factor, c, view)?;
        Some((lo.min(p), hi.max(p)))
    })
}

#[cfg(test)]
#[path = "space_tests.rs"]
mod space_tests;
