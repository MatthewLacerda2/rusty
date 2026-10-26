//! Multi-camera compositing (#93): the ordered stack of [`Camera`] passes the
//! renderer draws each frame. The camera itself is sim-side (`scene::camera`).

use crate::components::RenderTarget;
use crate::scene::camera::{active_cameras, render_order, screen_cameras};
use crate::scene::{camera_for_entity, Camera, Scene};

/// The screen's camera stack for one frame, and the entity whose settings post-FX
/// reads (#1434).
pub struct ScreenStack {
    /// The cameras, bottom to top; never empty.
    pub cameras: Vec<Camera>,
    /// The stack's base camera entity, `scene::camera::main_camera`: the lowest
    /// `render_order` active screen camera, the one `Graphics.*` edits.
    /// Its `CameraComponent` and `PostProcessingComponent` drive the view's post-FX,
    /// Unity URP's camera-stack rule; overlay cameras add no post-FX of their own. The
    /// edit-mode free camera has no entity, so the Scene view previews the Game view's
    /// base camera. `None` when the scene has no active screen camera.
    pub base_id: Option<u32>,
}

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
pub fn build_camera_stack(base: &Camera, scene: &Scene, is_playing: bool) -> ScreenStack {
    let ids = screen_cameras(scene);
    let base_id = ids.first().copied();
    let mut cameras: Vec<Camera> = match is_playing {
        true => ids
            .iter()
            .filter_map(|&id| camera_for_entity(base, scene, id))
            .collect(),
        false => Vec::new(),
    };
    match cameras.first_mut() {
        Some(first) => {
            (first.position, first.yaw, first.pitch) = (base.position, base.yaw, base.pitch)
        }
        None => cameras.push(base.clone()),
    }
    ScreenStack { cameras, base_id }
}

/// One camera that draws into a render texture this frame (#430).
pub struct TextureCamera {
    /// The camera's entity: its own settings drive this view's post-FX (#1434).
    pub id: u32,
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
