//! Multi-camera compositing (#93): the ordered stack of [`Camera`] passes the
//! renderer draws each frame. The camera itself is sim-side (`scene::camera`).

use crate::components::RenderTarget;
use crate::scene::{camera_for_entity, Camera, Scene};

/// Build the ordered camera stack the renderer composites for a frame (#93).
///
/// In play mode the scene's active screen [`CameraComponent`](crate::scene::CameraComponent)
/// entities drive rendering, sorted by `render_order` ascending so later cameras
/// composite over earlier ones — the Unity world-cam + viewmodel-cam + UI-cam stack.
/// The first (base) camera keeps `base`'s pose: the caller owns the main view (the
/// player and Game view derive it from that same entity; a script may drive it
/// through `Camera.*`). Every later camera renders from **its own entity's
/// transform** (#430) — a viewmodel camera parented to the player's head follows
/// it, a split view looks elsewhere. Each carries its own lens, projection, mask and
/// clear flags. Render-texture cameras are not on the screen stack (see
/// [`texture_cameras`]).
///
/// In edit mode (or when no active camera component exists) the free-fly `base`
/// camera is the single pass, preserving the editor viewport behavior.
pub fn build_camera_stack(base: &Camera, scene: &Scene, is_playing: bool) -> Vec<Camera> {
    if !is_playing {
        return vec![base.clone()];
    }
    let mut stack: Vec<(i32, u32)> = active_cameras(scene)
        .filter(|(_, target)| target.is_none())
        .map(|(id, _)| (order_of(scene, id), id))
        .collect();
    // Stable sort keeps authoring order among cameras sharing a render_order.
    stack.sort_by_key(|(order, _)| *order);
    let mut cams: Vec<Camera> = stack
        .iter()
        .filter_map(|&(_, id)| camera_for_entity(base, scene, id))
        .collect();
    match cams.first_mut() {
        Some(first) => {
            (first.position, first.yaw, first.pitch) = (base.position, base.yaw, base.pitch);
            cams
        }
        None => vec![base.clone()],
    }
}

/// One camera that draws into a render texture this frame (#430).
pub struct TextureCamera {
    pub camera: Camera,
    pub target: RenderTarget,
}

/// The scene's active render-texture cameras, each from its own entity's transform,
/// sorted by `render_order` (a later camera may show an earlier one's texture).
/// Two cameras naming one texture: the last in that order owns it.
pub fn texture_cameras(base: &Camera, scene: &Scene) -> Vec<TextureCamera> {
    let mut cams: Vec<(i32, TextureCamera)> = active_cameras(scene)
        .filter_map(|(id, target)| {
            let camera = camera_for_entity(base, scene, id)?;
            Some((
                order_of(scene, id),
                TextureCamera {
                    camera,
                    target: target?,
                },
            ))
        })
        .collect();
    cams.sort_by_key(|(order, _)| *order);
    cams.into_iter().map(|(_, c)| c).collect()
}

/// Every active camera entity with its render-texture target (if any).
fn active_cameras(scene: &Scene) -> impl Iterator<Item = (u32, Option<RenderTarget>)> + '_ {
    scene.world.ids_with_camera().into_iter().filter_map(|id| {
        let c = scene.world.camera(id)?;
        (scene.world.is_active(id) && c.active).then(|| (id, c.target_texture.clone()))
    })
}

fn order_of(scene: &Scene, id: u32) -> i32 {
    scene.world.camera(id).map_or(0, |c| c.render_order)
}

#[cfg(test)]
#[path = "camera_tests.rs"]
mod tests;
