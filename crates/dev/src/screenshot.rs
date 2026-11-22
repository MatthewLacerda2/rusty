use std::path::Path;

use super::capture::CaptureHost;
use rusty_app::GameWorld;
use rusty_scene::Camera;
use rusty_scene::Scene;

pub use rusty_core::settings::video::{DEFAULT_HEIGHT, DEFAULT_WIDTH};

pub fn capture_into(
    host: &mut CaptureHost,
    scene: &Scene,
    camera: &Camera,
    path: impl AsRef<Path>,
    width: u32,
    height: u32,
) -> Result<bool, String> {
    if host.draw(scene, camera, width, height)?.is_none() {
        return Ok(false);
    }
    host.write_png(path)?;
    Ok(true)
}

pub fn capture(
    scene: &Scene,
    camera: &Camera,
    path: impl AsRef<Path>,
    width: u32,
    height: u32,
) -> Result<bool, String> {
    capture_into(&mut CaptureHost::new(), scene, camera, path, width, height)
}

pub fn capture_world_into(
    host: &mut CaptureHost,
    world: &GameWorld,
    path: impl AsRef<Path>,
    width: u32,
    height: u32,
) -> Result<bool, String> {
    share_world(host, world, width, height);
    let camera = world.main_view();
    let scene = world.scene().borrow();
    capture_into(host, &scene, &camera, path, width, height)
}

pub fn share_world(host: &mut CaptureHost, world: &GameWorld, width: u32, height: u32) {
    if let Some(renderer) = host.renderer(width, height) {
        let textures = world.resources.screen.borrow().textures().clone();
        renderer.set_ui_textures(textures);
        let display_scale = world.resources.screen.borrow().display_scale();
        renderer.apply_graphics(
            *world.script_manager().graphics_cell().borrow(),
            display_scale,
        );
    }
}
