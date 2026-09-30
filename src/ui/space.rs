//! src/ui/space.rs — where a canvas lives: on the screen, or on a plane in the world (#429).
//!
//! Every canvas lays out in its own reference units (`layout`); this module says
//! where those units end up. A [`CanvasSpace::Screen`] canvas is the overlay: a
//! reference unit is `scale_factor` screen pixels. A [`CanvasSpace::World`] canvas
//! is a plane in the scene, reached through one matrix from reference units to
//! world space — the renderer draws through it and the hit-test intersects the
//! pointer's camera ray with it, so what is drawn is what is hit.
//!
//! - `WorldSpace`: centred on the canvas entity's world transform, facing its +Z,
//!   `pixels_per_unit` reference units to the metre.
//! - `ScreenSpaceCamera`: a plane `plane_distance` in front of the camera that fills
//!   the view there, tilted about its centre and swung about the eye by the sway
//!   lag (`ui::sway`). Without a camera it is the overlay.
//!
//! [`UiView`] is what the UI is computed against — the screen size (a sim input)
//! and the camera looking through it — and makes the [`UiPointer`] a tick's input
//! hit-tests with: the screen point, and the camera ray through it (the screen
//! centre while the cursor is locked, for "look at it and press E" terminals).

use glam::{Mat4, Quat, Vec2, Vec3};

use crate::components::{CanvasComponent, CanvasRenderMode};
use crate::ecs::World;
use crate::scene::Camera;
use crate::ui::UiRect;

/// The screen and the camera the UI is laid out, drawn and hit-tested against.
#[derive(Clone)]
pub struct UiView {
    /// The screen size in pixels (see `ScreenSize`).
    pub screen: Vec2,
    /// The camera looking through it; `None` places no world UI and no marker.
    pub camera: Option<Camera>,
}

impl UiView {
    /// A view with no camera: overlay layout only.
    pub fn screen(screen: Vec2) -> Self {
        Self {
            screen: screen.max(Vec2::ONE),
            camera: None,
        }
    }

    /// A view through `camera`.
    pub fn with_camera(screen: Vec2, camera: Camera) -> Self {
        Self {
            screen: screen.max(Vec2::ONE),
            camera: Some(camera),
        }
    }

    /// The pointer for a mouse at `mouse` (UI screen pixels), or a locked cursor
    /// (`None`): off the screen canvases, its ray through the screen centre.
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

/// Where the pointer is this tick: on the screen canvases (`screen`, UI pixels) and
/// in the world (`ray`, origin + unit direction, reaching `ray_limit` metres — the
/// nearest wall, so a world canvas behind one is not clicked through it).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UiPointer {
    pub screen: Option<Vec2>,
    pub ray: Option<(Vec3, Vec3)>,
    pub ray_limit: f32,
}

impl UiPointer {
    /// A pointer on the screen only (no camera ray): overlay canvases.
    pub fn at(screen: Vec2) -> Self {
        Self {
            screen: Some(screen),
            ray: None,
            ray_limit: f32::INFINITY,
        }
    }
}

/// Where one canvas's reference units end up.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum CanvasSpace {
    /// On the screen: `scale_factor` pixels per reference unit.
    #[default]
    Screen,
    /// On a plane in the world: reference units `(x, y, 0)` → world space.
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
            CanvasSpace::World(world_matrix(world, id) * units * centre)
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

/// Where `ray` crosses the plane of `to_world` (a [`CanvasSpace::World`] matrix):
/// the point in the canvas's reference units and the distance along the ray. `None`
/// when the plane is behind the ray's origin or parallel to it. Both faces hit.
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

/// Where a canvas point (reference units) lands on the screen, in UI pixels — or
/// `None` for a world canvas point behind the camera. `scale` is the canvas's scale
/// factor.
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

/// The screen-pixel bounds `(min, max)` of `rect`'s final quad in `space` — the
/// projected quad for a world canvas, `None` when a corner is behind the camera.
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

/// An entity's world matrix, walking its parent chain (bounded by the entity count,
/// so a malformed cycle cannot hang it).
pub fn world_matrix(world: &World, id: u32) -> Mat4 {
    let mut m = Mat4::IDENTITY;
    let mut cur = Some(id);
    for _ in 0..=world.len() {
        let Some(c) = cur else { break };
        if let Some(t) = world.transform(c) {
            m = t.to_matrix() * m;
        }
        cur = world.parent_id(c);
    }
    m
}

#[cfg(test)]
#[path = "space_tests.rs"]
mod space_tests;
