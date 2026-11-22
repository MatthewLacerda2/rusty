use crate::Scene;
use rusty_components::{PostProcessingComponent, RenderTarget};
use rusty_ecs::CompRef;

pub fn active_cameras(scene: &Scene) -> impl Iterator<Item = (u32, Option<RenderTarget>)> + '_ {
    scene.world.ids_with_camera().into_iter().filter_map(|id| {
        let c = scene.world.camera(id)?;
        (scene.world.is_active_in_hierarchy(id) && c.active).then(|| (id, c.target_texture.clone()))
    })
}

pub fn render_order(scene: &Scene, id: u32) -> i32 {
    scene.world.camera(id).map_or(0, |c| c.render_order)
}

pub fn screen_cameras(scene: &Scene) -> Vec<u32> {
    let mut ids: Vec<(i32, u32)> = active_cameras(scene)
        .filter(|(_, target)| target.is_none())
        .map(|(id, _)| (render_order(scene, id), id))
        .collect();
    // Stable: cameras sharing a `render_order` keep entity order.
    ids.sort_by_key(|(order, _)| *order);
    ids.into_iter().map(|(_, id)| id).collect()
}

pub fn main_camera(scene: &Scene) -> Option<u32> {
    screen_cameras(scene).first().copied()
}

pub fn camera_volume(scene: &Scene, id: u32) -> Option<CompRef<'_, PostProcessingComponent>> {
    let vc = scene.world.post_processing(id)?;
    (scene.world.is_active_in_hierarchy(id) && vc.active).then_some(vc)
}
