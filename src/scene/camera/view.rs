//! The main view's pose (#1619): one answer to "where is the screen looking from".
//!
//! Following Unity, where `Camera.main` is a GameObject: when the scene has a
//! [`main_camera`] entity, the main view *is* that entity's world transform, on every
//! path (the player, the editor's Game view, the headless harness and screenshots).
//! `Camera.SetPosition` / `SetYaw` / `SetPitch` move that entity, `Camera.SetFov` sets
//! its lens, and the view
//! [`Camera`] resource mirrors its pose for the sim's other readers (the audio
//! listener, world-space UI). Without a camera entity, the resource is the free view
//! camera and `Camera.*` drives it alone.

use glam::{Quat, Vec3};

use super::{game_camera_from_scene, held_fov, main_camera, Camera, MAX_PITCH};
use crate::scene::authoring::rendering::camera as camera_ops;
use crate::scene::Scene;

/// Copy the main camera entity's pose onto the view `camera`; without one, the free
/// camera keeps its own.
pub fn sync_pose_from_scene(camera: &mut Camera, scene: &Scene) {
    let view = game_camera_from_scene(camera, scene);
    (camera.position, camera.yaw, camera.pitch) = (view.position, view.yaw, view.pitch);
}

/// Move the main view to world `position`: the main camera entity when there is
/// one, and the view `camera` either way.
pub fn set_view_position(camera: &mut Camera, scene: &mut Scene, position: Vec3) {
    camera.position = position;
    if let Some(id) = main_camera(scene) {
        scene.world.set_world_position(id, position);
    }
}

/// Turn the main view to `yaw` / `pitch` (degrees; pitch held to ±[`MAX_PITCH`]).
/// The entity's world rotation is rebuilt from the two, so any roll it had is dropped.
pub fn set_view_orientation(camera: &mut Camera, scene: &mut Scene, yaw: f32, pitch: f32) {
    let pitch = pitch.clamp(-MAX_PITCH, MAX_PITCH);
    (camera.yaw, camera.pitch) = (yaw, pitch);
    if let Some(id) = main_camera(scene) {
        scene
            .world
            .set_world_rotation(id, look_rotation(yaw, pitch));
    }
}

/// Set the main view's vertical field of view (degrees, held to the lens bounds):
/// the main camera entity's `fieldOfView` when there is one (Unity's
/// `Camera.main.fieldOfView`), and the view `camera` either way.
pub fn set_view_fov(camera: &mut Camera, scene: &mut Scene, fov: f32) {
    camera.fov = held_fov(fov);
    if let Some(mut c) = main_camera(scene).and_then(|id| scene.world.camera_mut(id)) {
        camera_ops::set_fov(&mut c, fov);
    }
}

/// The world rotation of an entity that looks along [`Camera::forward`] for `yaw` /
/// `pitch`: the inverse of `yaw_pitch_from_matrix`. An entity looks down its local
/// -Z, so yaw 0 (looking down +X) is a -90° turn about Y.
pub fn look_rotation(yaw: f32, pitch: f32) -> Quat {
    Quat::from_rotation_y((-90.0 - yaw).to_radians()) * Quat::from_rotation_x(pitch.to_radians())
}

#[cfg(test)]
#[path = "view_tests.rs"]
mod tests;
