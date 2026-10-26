//! Multi-camera compositing (#93): the ordered stack of [`Camera`] passes the
//! renderer draws each frame. The camera itself is sim-side (`scene::camera`).

use crate::components::RenderTarget;
use crate::scene::{camera_for_entity, Camera, Scene};

/// The screen's camera stack for one frame, and the entity whose settings post-FX
/// reads (#1434).
pub struct ScreenStack {
    /// The cameras, bottom to top; never empty.
    pub cameras: Vec<Camera>,
    /// The stack's base camera entity: the lowest `render_order` active screen camera.
    /// Its `CameraComponent` and `VisualCorrectionComponent` drive the view's post-FX,
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
    let mut ids: Vec<(i32, u32)> = active_cameras(scene)
        .filter(|(_, target)| target.is_none())
        .map(|(id, _)| (order_of(scene, id), id))
        .collect();
    // Stable sort keeps authoring order among cameras sharing a render_order.
    ids.sort_by_key(|(order, _)| *order);
    let base_id = ids.first().map(|&(_, id)| id);
    let mut cameras: Vec<Camera> = match is_playing {
        true => ids
            .iter()
            .filter_map(|&(_, id)| camera_for_entity(base, scene, id))
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
                order_of(scene, id),
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

/// Every active camera entity with its render-texture target (if any).
fn active_cameras(scene: &Scene) -> impl Iterator<Item = (u32, Option<RenderTarget>)> + '_ {
    scene.world.ids_with_camera().into_iter().filter_map(|id| {
        let c = scene.world.camera(id)?;
        (scene.world.is_active_in_hierarchy(id) && c.active).then(|| (id, c.target_texture.clone()))
    })
}

fn order_of(scene: &Scene, id: u32) -> i32 {
    scene.world.camera(id).map_or(0, |c| c.render_order)
}

#[cfg(test)]
#[path = "camera_tests.rs"]
mod tests;
