//! src/api/ui/view.rs — the screen + camera the `UI` verbs compute against (#429).
//!
//! The same [`UiView`] the per-tick systems build (`app::systems::ui`): the sim's screen
//! size and texture sizes and the main view (the camera entity's pose and lens when the
//! scene has one, #1619), so `UI.GetRect`, `UI.Raycast`, `UI.Click` and
//! `UI.List` see markers and world canvases exactly where they draw and hit.

use std::cell::RefCell;

use glam::Vec2;

use crate::core::settings::video::VideoSettings;
use crate::physics::PhysicsWorld;
use crate::scene::{game_camera_from_scene, Camera, Scene};
use crate::ui::events::{WALL_RANGE, WALL_TOLERANCE};
use crate::ui::{ScreenSize, UiPointer, UiView};

/// The view through the main camera on the sim's screen.
pub(super) fn ui_view(
    screen: &RefCell<ScreenSize>,
    video: &RefCell<VideoSettings>,
    camera: &RefCell<Camera>,
    scene: &Scene,
) -> UiView {
    let camera = game_camera_from_scene(&camera.borrow(), scene);
    screen.borrow().view(&video.borrow(), Some(camera))
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
