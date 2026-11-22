use std::rc::Rc;

use super::cache::DirtCache;
use crate::gpu::textures::TextureKind;
use crate::{GpuTexture, Renderer};
use rusty_components::is_render_texture;
use rusty_components::PostProcessingComponent;
use rusty_scene::Scene;

impl Renderer {
    pub(crate) fn load_lens_dirt(&mut self, scene: &Scene) {
        let mut cache: DirtCache = std::mem::take(&mut self.post_passes.bloom.dirt);
        cache.refresh();
        let world = &scene.world;
        for id in world.ids_with_post_processing() {
            let Some(vc) = world.post_processing(id) else {
                continue;
            };
            if let Some(setting) = vc.bloom.dirt_texture.as_deref() {
                cache.request(setting, |file| self.upload_map(file, TextureKind::Colour));
            }
        }
        self.post_passes.bloom.dirt = cache;
    }

    pub(crate) fn lens_dirt(&self, vc: &PostProcessingComponent) -> Option<Rc<GpuTexture>> {
        let setting = vc.bloom.dirt_texture.as_deref()?;
        if is_render_texture(setting) {
            return self.gpu_textures.get(setting).cloned();
        }
        self.post_passes.bloom.dirt.get(setting)
    }
}

pub(crate) fn render_textures(scene: &Scene) -> Vec<String> {
    let world = &scene.world;
    (world.ids_with_post_processing().into_iter())
        .filter(|&id| world.is_active_in_hierarchy(id))
        .filter_map(|id| world.post_processing(id)?.bloom.dirt_texture.clone())
        .filter(|path| is_render_texture(path))
        .collect()
}
