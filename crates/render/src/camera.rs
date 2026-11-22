use crate::interpolate::DrawPoses;
use rusty_components::RenderTarget;
use rusty_scene::camera::{active_cameras, render_order, screen_cameras};
use rusty_scene::{camera_for_entity, Camera, Scene};

fn entity_camera(base: &Camera, scene: &Scene, id: u32, poses: &DrawPoses) -> Option<Camera> {
    let mut camera = camera_for_entity(base, scene, id)?;
    poses.pose_camera(&mut camera, scene, id);
    Some(camera)
}

pub struct ScreenStack {
    pub cameras: Vec<Camera>,
    pub entities: Vec<Option<u32>>,
    pub base_id: Option<u32>,
}

/// The base camera keeps `base`'s pose, which `Renderer::render` has already blended; the rest pose
/// by `poses`.
pub fn build_camera_stack(
    base: &Camera,
    scene: &Scene,
    is_playing: bool,
    poses: &DrawPoses,
) -> ScreenStack {
    let ids = screen_cameras(scene);
    let base_id = ids.first().copied();
    let mut stack: Vec<(Option<u32>, Camera)> = match is_playing {
        true => ids
            .iter()
            .filter_map(|&id| Some((Some(id), entity_camera(base, scene, id, poses)?)))
            .collect(),
        false => Vec::new(),
    };
    match stack.first_mut() {
        Some((_, first)) => {
            (first.position, first.yaw, first.pitch) = (base.position, base.yaw, base.pitch)
        }
        None => stack.push((None, base.clone())),
    }
    let (entities, cameras) = stack.into_iter().unzip();
    ScreenStack {
        cameras,
        entities,
        base_id,
    }
}

pub struct TextureCamera {
    pub id: u32,
    pub camera: Camera,
    pub target: RenderTarget,
}

pub fn texture_cameras(base: &Camera, scene: &Scene, poses: &DrawPoses) -> Vec<TextureCamera> {
    let mut cams: Vec<(i32, TextureCamera)> = active_cameras(scene)
        .filter_map(|(id, target)| {
            let camera = entity_camera(base, scene, id, poses)?;
            Some((
                render_order(scene, id),
                TextureCamera {
                    id,
                    camera,
                    target: target?,
                },
            ))
        })
        .collect();
    cams.sort_by_key(|(order, _)| *order);
    cams.into_iter().map(|(_, c)| c).collect()
}

#[cfg(test)]
#[path = "camera_tests.rs"]
mod tests;
