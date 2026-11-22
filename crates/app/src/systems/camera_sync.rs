use rusty_scene::camera::sync_pose_from_scene;
use rusty_scene::{game_camera_from_scene, sync_lens_from_scene, Camera};

use crate::GameWorld;

impl GameWorld {
    pub fn main_view(&self) -> Camera {
        game_camera_from_scene(&self.resources.camera.borrow(), &self.world.scene.borrow())
    }

    pub(crate) fn sync_render_camera(&mut self) {
        let scene = self.world.scene.borrow();
        let mut camera = self.resources.camera.borrow_mut();
        sync_pose_from_scene(&mut camera, &scene);
        sync_lens_from_scene(&mut camera, &scene, scene.is_playing());
    }

    pub(crate) fn sync_shader_time(&mut self) {
        let (time, ui_time) = if self.world.scene.borrow().is_playing() {
            let t = self.resources.time.borrow();
            (t.time as f32, t.unscaled_time as f32)
        } else {
            (0.0, 0.0)
        };
        let mut scene = self.world.scene.borrow_mut();
        scene.shader_time = time;
        scene.ui_time = ui_time;
    }
}

#[cfg(test)]
#[path = "shader_time_tests.rs"]
mod shader_time_tests;
