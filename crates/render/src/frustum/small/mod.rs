use glam::{Mat4, Vec3};

use crate::lod::{max_scale, relative_height};
use rusty_components::{MeshComponent, Projection};
use rusty_scene::Camera;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct SmallObjectCull {
    lens: Option<(Vec3, f32, Projection)>,
}

impl SmallObjectCull {
    pub(crate) fn from_camera(cam: &Camera) -> Self {
        Self {
            lens: Some((cam.position, cam.fov, cam.projection)),
        }
    }

    pub(crate) fn off() -> Self {
        Self { lens: None }
    }

    #[cfg(test)]
    pub(crate) fn culls(
        &self,
        mesh: &MeshComponent,
        centre: Vec3,
        local: (Vec3, Vec3),
        world: Mat4,
    ) -> bool {
        self.culls_by(SmallLimits::of(mesh), centre, local, world)
    }

    pub(crate) fn culls_by(
        &self,
        limits: SmallLimits,
        centre: Vec3,
        (min, max): (Vec3, Vec3),
        world: Mat4,
    ) -> bool {
        let Some((eye, fov, projection)) = self.lens else {
            return false;
        };
        let distance = centre.distance(eye);
        if limits.max_draw_distance > 0.0 && distance > limits.max_draw_distance {
            return true;
        }
        limits.min_screen_size > 0.0 && {
            let size = (max - min).max_element() * max_scale(world);
            relative_height(size, distance, fov, projection) < limits.min_screen_size
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct SmallLimits {
    pub max_draw_distance: f32,
    pub min_screen_size: f32,
}

impl SmallLimits {
    pub(crate) fn of(mesh: &MeshComponent) -> Self {
        Self {
            max_draw_distance: mesh.max_draw_distance,
            min_screen_size: mesh.min_screen_size,
        }
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod gpu_tests;

#[cfg(test)]
mod lod_gpu_tests;
