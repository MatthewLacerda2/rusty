//! Which camera entity is the screen's (#1434): one answer for the renderer's screen
//! stack, `Graphics.*`, the Game view and the view camera's lens sync, so a
//! render-texture camera never stands in for the screen's.

use crate::components::{PostProcessingComponent, RenderTarget};
use crate::ecs::CompRef;
use crate::scene::Scene;

/// Every active camera entity (active in the hierarchy, its component active) with
/// its render-texture target, `None` for a screen camera.
pub fn active_cameras(scene: &Scene) -> impl Iterator<Item = (u32, Option<RenderTarget>)> + '_ {
    scene.world.ids_with_camera().into_iter().filter_map(|id| {
        let c = scene.world.camera(id)?;
        (scene.world.is_active_in_hierarchy(id) && c.active).then(|| (id, c.target_texture.clone()))
    })
}

/// A camera entity's `render_order`, 0 without a camera.
pub fn render_order(scene: &Scene, id: u32) -> i32 {
    scene.world.camera(id).map_or(0, |c| c.render_order)
}

/// The active screen cameras, bottom to top: ascending `render_order`, ties in
/// entity order. Render-texture cameras are not on the screen.
pub fn screen_cameras(scene: &Scene) -> Vec<u32> {
    let mut ids: Vec<(i32, u32)> = active_cameras(scene)
        .filter(|(_, target)| target.is_none())
        .map(|(id, _)| (render_order(scene, id), id))
        .collect();
    // Stable sort keeps entity order among cameras sharing a render_order.
    ids.sort_by_key(|(order, _)| *order);
    ids.into_iter().map(|(_, id)| id).collect()
}

/// The screen stack's base camera, Unity's `Camera.main`: the lowest `render_order`
/// active camera that renders to the screen. Its settings drive the screen's post-FX
/// and shadows, and `Graphics.*` edits them. `None` without a screen camera.
pub fn main_camera(scene: &Scene) -> Option<u32> {
    screen_cameras(scene).first().copied()
}

/// `id`'s post-processing volume, when the entity is active in the hierarchy and
/// the volume is active: the settings a view whose camera is `id` runs.
pub fn camera_volume(scene: &Scene, id: u32) -> Option<CompRef<'_, PostProcessingComponent>> {
    let vc = scene.world.post_processing(id)?;
    (scene.world.is_active_in_hierarchy(id) && vc.active).then_some(vc)
}
