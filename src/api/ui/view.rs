//! src/api/ui/view.rs — the screen + camera the `UI` verbs compute against (#429).
//!
//! The same [`UiView`] the per-tick systems build (`app::ui`): the sim's screen
//! size and the active camera, so `UI.GetRect`, `UI.Raycast`, `UI.Click` and
//! `UI.List` see markers and world canvases exactly where they draw and hit.

use std::cell::RefCell;

use glam::Vec2;

use crate::core::video::VideoSettings;
use crate::physics::PhysicsWorld;
use crate::scene::Camera;
use crate::ui::events::{WALL_RANGE, WALL_TOLERANCE};
use crate::ui::{ScreenSize, UiPointer, UiView};

/// The view through the active camera on the sim's screen.
pub(super) fn ui_view(
    screen: &RefCell<ScreenSize>,
    video: &RefCell<VideoSettings>,
    camera: &RefCell<Camera>,
) -> UiView {
    let px = screen.borrow().pixels(&video.borrow());
    UiView::with_camera(px, camera.borrow().clone())
}

/// The pointer at `mouse` (UI pixels; `None`: a locked cursor, the centre ray),
/// its ray stopped at the first solid collider, as the event system stops it.
pub(super) fn pointer(
    view: &UiView,
    physics: &RefCell<Option<PhysicsWorld>>,
    mouse: Option<Vec2>,
) -> UiPointer {
    let mut pointer = view.pointer(mouse);
    if let (Some((origin, dir)), Some(physics)) = (pointer.ray, physics.borrow().as_ref()) {
        pointer.ray_limit = physics
            .first_surface_ahead(origin, dir, WALL_RANGE)
            .map_or(f32::INFINITY, |h| h.distance + WALL_TOLERANCE);
    }
    pointer
}
