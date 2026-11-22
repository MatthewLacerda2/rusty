use std::cell::RefCell;

use glam::Vec2;

use rusty_core::settings::video::VideoSettings;
use rusty_physics::PhysicsWorld;
use rusty_scene::{game_camera_from_scene, Camera, Scene};
use rusty_ui::events::{WALL_RANGE, WALL_TOLERANCE};
use rusty_ui::{ScreenSize, UiPointer, UiView};

/// Built as `app`'s `Resources::ui_view` builds the frame's, so the verbs hit what draws: change
/// both together.
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
