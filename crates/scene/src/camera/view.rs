use glam::{Quat, Vec3};

use super::{game_camera_from_scene, held_fov, main_camera, Camera, MAX_PITCH};
use crate::Scene;

pub fn sync_pose_from_scene(camera: &mut Camera, scene: &Scene) {
    let view = game_camera_from_scene(camera, scene);
    (camera.position, camera.yaw, camera.pitch) = (view.position, view.yaw, view.pitch);
}

pub fn set_view_position(camera: &mut Camera, scene: &mut Scene, position: Vec3) {
    camera.position = position;
    if let Some(id) = main_camera(scene) {
        scene.world.set_world_position(id, position);
    }
}

pub fn set_view_orientation(camera: &mut Camera, scene: &mut Scene, yaw: f32, pitch: f32) {
    let pitch = pitch.clamp(-MAX_PITCH, MAX_PITCH);
    (camera.yaw, camera.pitch) = (yaw, pitch);
    if let Some(id) = main_camera(scene) {
        scene
            .world
            .set_world_rotation(id, look_rotation(yaw, pitch));
    }
}

pub fn set_view_fov(camera: &mut Camera, scene: &mut Scene, fov: f32) {
    camera.fov = held_fov(fov);
    if let Some(mut c) = main_camera(scene).and_then(|id| scene.world.camera_mut(id)) {
        c.fov = held_fov(fov);
    }
}

/// The inverse of `yaw_pitch_from_matrix`: an entity looks down -Z, so yaw 0 (+X) is -90° about
/// Y.
pub fn look_rotation(yaw: f32, pitch: f32) -> Quat {
    Quat::from_rotation_y((-90.0 - yaw).to_radians()) * Quat::from_rotation_x(pitch.to_radians())
}

#[cfg(test)]
#[path = "view_tests.rs"]
mod tests;
