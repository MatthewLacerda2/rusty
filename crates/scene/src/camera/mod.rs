use glam::camera::rh::proj::directx;
use glam::{Mat4, Vec3};

mod history;
mod lens;
mod main;
mod projection;
mod view;

pub use history::HistoryCuts;
pub use lens::{held_fov, held_lens, Lens, MAX_LENS_EXTENT};
pub use main::{active_cameras, camera_volume, main_camera, render_order, screen_cameras};
pub use projection::ScreenPoint;
pub use view::{
    look_rotation, set_view_fov, set_view_orientation, set_view_position, sync_pose_from_scene,
};

use crate::{layer_in_mask, ClearFlags, Scene};
use rusty_components::Projection;

#[derive(Clone)]
pub struct Camera {
    pub position: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub fov: f32,
    pub near: f32,
    pub far: f32,
    pub culling_mask: u32,
    pub clear_flags: ClearFlags,
    pub projection: Projection,
    pub use_occlusion_culling: bool,
    pub up: Vec3,
    scene_lens: bool,
}

pub use rusty_components::rendering::camera::DEFAULT_FOV;
use rusty_components::rendering::camera::{DEFAULT_FAR, DEFAULT_NEAR};

pub const MAX_PITCH: f32 = 89.0;

impl Camera {
    pub fn new(position: Vec3, yaw: f32, pitch: f32) -> Self {
        Self {
            position,
            yaw,
            pitch,
            fov: DEFAULT_FOV,
            near: DEFAULT_NEAR,
            far: DEFAULT_FAR,
            culling_mask: u32::MAX,
            clear_flags: ClearFlags::Skybox,
            projection: Projection::Perspective,
            use_occlusion_culling: true,
            up: Vec3::Y,
            scene_lens: false,
        }
    }

    pub fn forward(&self) -> Vec3 {
        let pitch_rad = self.pitch.to_radians();
        let yaw_rad = self.yaw.to_radians();

        let cos_pitch = pitch_rad.cos();
        let sin_pitch = pitch_rad.sin();
        let cos_yaw = yaw_rad.cos();
        let sin_yaw = yaw_rad.sin();

        Vec3::new(cos_yaw * cos_pitch, sin_pitch, sin_yaw * cos_pitch).normalize()
    }

    pub fn right(&self) -> Vec3 {
        self.forward().cross(self.up).normalize()
    }

    pub fn build_view_projection(&self, aspect: f32) -> Mat4 {
        self.projection_matrix(aspect) * self.view_matrix()
    }

    pub fn projection_matrix(&self, aspect: f32) -> Mat4 {
        let (near, far) = (self.near, self.far);
        match self.projection {
            Projection::Perspective => {
                directx::perspective(self.fov.to_radians(), aspect, near, far)
            }
            Projection::Orthographic { size: h } => {
                directx::orthographic(-h * aspect, h * aspect, -h, h, near, far)
            }
        }
    }

    pub fn renders_layer(&self, layer: u8) -> bool {
        layer_in_mask(layer, self.culling_mask)
    }
}

pub fn sync_lens_from_scene(camera: &mut Camera, scene: &Scene, is_playing: bool) {
    let lens = is_playing
        .then(|| main_camera(scene))
        .flatten()
        .and_then(|id| scene.world.camera(id))
        .map(|c| (held_lens(&c), c.culling_mask, c.use_occlusion_culling));

    let had_scene_lens = std::mem::replace(&mut camera.scene_lens, lens.is_some());
    match lens {
        Some((lens, mask, occlusion)) => {
            lens.apply(camera);
            camera.culling_mask = mask;
            camera.use_occlusion_culling = occlusion;
        }
        None => {
            if had_scene_lens {
                camera.fov = DEFAULT_FOV;
            }
            camera.projection = Projection::Perspective;
            camera.near = DEFAULT_NEAR;
            camera.far = DEFAULT_FAR;
            camera.culling_mask = u32::MAX;
            camera.use_occlusion_culling = true;
        }
    }
}

pub fn game_camera_from_scene(base: &Camera, scene: &Scene) -> Camera {
    main_camera(scene)
        .and_then(|id| camera_for_entity(base, scene, id))
        .unwrap_or_else(|| base.clone())
}

pub fn camera_for_entity(base: &Camera, scene: &Scene, id: u32) -> Option<Camera> {
    let c = scene.world.camera(id)?;
    let mut cam = base.clone();
    pose_at(&mut cam, scene.world.local_to_world(id));
    apply_lens(&mut cam, &c);
    Some(cam)
}

pub fn pose_at(cam: &mut Camera, world: Mat4) {
    let (yaw, pitch) = yaw_pitch_from_matrix(world);
    cam.up = Vec3::Y;
    cam.position = world.col(3).truncate();
    cam.yaw = yaw;
    cam.pitch = pitch;
}

pub fn apply_lens(cam: &mut Camera, c: &rusty_components::CameraComponent) {
    held_lens(c).apply(cam);
    cam.culling_mask = c.culling_mask;
    cam.use_occlusion_culling = c.use_occlusion_culling;
    cam.clear_flags = c.clear_flags;
}

#[cfg(test)]
#[path = "sync_tests.rs"]
mod sync_tests;

fn yaw_pitch_from_matrix(world: Mat4) -> (f32, f32) {
    let forward = (-world.col(2).truncate()).normalize_or_zero();
    let pitch = forward.y.clamp(-1.0, 1.0).asin().to_degrees();
    let heading = if forward.x.hypot(forward.z) < VERTICAL_EPSILON {
        world.col(1).truncate() * -forward.y.signum()
    } else {
        forward
    };
    let yaw = heading.z.atan2(heading.x).to_degrees();
    (yaw, pitch)
}

const VERTICAL_EPSILON: f32 = 0.01;
