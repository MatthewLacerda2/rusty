//! Bloom's lens dirt (#1442), Unity's *Dirt Texture* / *Dirt Intensity*: a texture
//! cropped to cover the screen that the composite multiplies by the bloom, so a
//! smudge only shows where bright light reaches it. The texture is loaded into
//! bloom's own cache ([`DirtCache`]) before the frame's passes, whenever a volume
//! names one, even at a dirt intensity of `0`, so tweening it up never decodes on
//! that frame; the bloom pass then picks the view's, and the composite binds it as
//! bloom's composite texture (#1473), or the registry's black texel when there is
//! none. An `rt:` dirt is a render
//! texture, which the frame draws because a volume names it.

use std::rc::Rc;

use super::cache::DirtCache;
use crate::components::is_render_texture;
use crate::components::PostProcessingComponent;
use crate::render::gpu::textures::TextureKind;
use crate::render::{GpuTexture, Renderer};
use crate::scene::Scene;

impl Renderer {
    /// Keep bloom's dirt cache in step with the disk, then read the dirt texture of
    /// every volume in `scene` that names one and hasn't been read.
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

    /// The dirt texture `vc` names, when it loaded (or, for `rt:`, is drawn).
    pub(crate) fn lens_dirt(&self, vc: &PostProcessingComponent) -> Option<Rc<GpuTexture>> {
        let setting = vc.bloom.dirt_texture.as_deref()?;
        if is_render_texture(setting) {
            return self.gpu_textures.get(setting).cloned();
        }
        self.post_passes.bloom.dirt.get(setting)
    }
}

/// Every `rt:` dirt texture an active volume names: render textures worth drawing
/// (`render_texture::referenced_render_textures`).
pub(crate) fn render_textures(scene: &Scene) -> Vec<String> {
    let world = &scene.world;
    (world.ids_with_post_processing().into_iter())
        .filter(|&id| world.is_active_in_hierarchy(id))
        .filter_map(|id| world.post_processing(id)?.bloom.dirt_texture.clone())
        .filter(|path| is_render_texture(path))
        .collect()
}
