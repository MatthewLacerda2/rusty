use std::collections::HashSet;

use glam::Mat4;

use rusty_components::{LodGroupComponent, Projection};
use rusty_scene::{Camera, Scene};

#[derive(Debug, Default)]
pub(crate) struct LodSelection {
    hidden: HashSet<u32>,
}

impl LodSelection {
    pub(crate) fn for_camera(scene: &Scene, cam: &Camera) -> Self {
        Self::build(scene, |group, world| {
            let height = screen_height(group.size * max_scale(world), cam, world);
            group.level_at(height)
        })
    }

    pub(crate) fn finest(scene: &Scene) -> Self {
        Self::build(scene, |_, _| Some(0))
    }

    pub(crate) fn hides(&self, id: u32) -> bool {
        self.hidden.contains(&id)
    }

    fn build(scene: &Scene, pick: impl Fn(&LodGroupComponent, Mat4) -> Option<usize>) -> Self {
        let mut hidden = HashSet::new();
        for id in scene.world.ids_with_lod_group() {
            if !scene.world.is_active_in_hierarchy(id) {
                continue;
            }
            let Some(group) = scene.world.lod_group(id) else {
                continue;
            };
            let shown =
                pick(&group, scene.world.local_to_world(id)).and_then(|i| group.levels.get(i));
            let keep = |r: &u32| shown.is_some_and(|l| l.renderers.contains(r));
            hidden.extend(group.renderers().filter(|r| !keep(r)));
        }
        Self { hidden }
    }
}

pub(crate) fn screen_height(size: f32, cam: &Camera, world: Mat4) -> f32 {
    let distance = world.w_axis.truncate().distance(cam.position);
    relative_height(size, distance, cam.fov, cam.projection)
}

pub(crate) fn relative_height(size: f32, distance: f32, fov: f32, projection: Projection) -> f32 {
    if let Projection::Orthographic { size: half } = projection {
        return size / (2.0 * half);
    }
    let half_fov = (fov.to_radians() * 0.5).tan();
    match distance * half_fov {
        d if d > 0.0 => size / (2.0 * d),
        _ => f32::INFINITY,
    }
}

pub(crate) fn max_scale(world: Mat4) -> f32 {
    let axes = [world.x_axis, world.y_axis, world.z_axis];
    axes.iter()
        .map(|a| a.truncate().length())
        .fold(0.0, f32::max)
}

#[cfg(test)]
#[path = "lod_tests.rs"]
mod lod_tests;

#[cfg(test)]
#[path = "lod_gpu_tests.rs"]
mod lod_gpu_tests;
