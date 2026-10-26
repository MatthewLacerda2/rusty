//! Bloom's lens dirt (#1442), Unity's *Dirt Texture* / *Dirt Intensity*: a texture
//! stretched over the screen that the composite multiplies by the bloom, so a
//! smudge only shows where bright light reaches it. The texture is loaded like any
//! other (`Renderer::load_texture`, cached and kept in step with the disk), before
//! the frame's passes ([`Renderer::load_lens_dirt`]); the bloom pass then picks the
//! view's from the cache, and the composite binds it (binding 10), or a black texel
//! when there is none.

use std::rc::Rc;

use crate::asset::MapRef;
use crate::components::rendering::post_processing::DEFAULT_DIRT_TEXTURE;
use crate::components::PostProcessingComponent;
use crate::render::{GpuTexture, Renderer};
use crate::scene::Scene;

/// The file `vc`'s dirt texture reads from, when it names one and any of it
/// applies: [`DEFAULT_DIRT_TEXTURE`] is the engine's own, wherever `engine/` is.
pub(crate) fn file(vc: &PostProcessingComponent) -> Option<MapRef> {
    let bloom = &vc.bloom;
    let path = bloom
        .dirt_texture
        .as_deref()
        .filter(|_| bloom.dirt_intensity > 0.0)?;
    Some(
        match path.strip_prefix("engine/") {
            Some(rel) if path == DEFAULT_DIRT_TEXTURE => crate::core::project::engine_path(rel),
            _ => path.to_string(),
        }
        .into(),
    )
}

impl Renderer {
    /// Load the dirt texture of every volume in `scene` that names one, logging
    /// once when it is missing or unreadable (it is retried when the file appears).
    pub(crate) fn load_lens_dirt(&mut self, scene: &Scene) {
        let world = &scene.world;
        let files: Vec<MapRef> = (world.ids_with_post_processing().into_iter())
            .filter_map(|id| file(&*world.post_processing(id)?))
            .collect();
        for map in files {
            let key = map.as_str();
            if self.gpu_textures.contains_key(key) || self.texture_freshness.is_miss(key) {
                continue;
            }
            self.load_texture(&map);
            if !self.gpu_textures.contains_key(key) {
                log::warn!("[Render] Bloom dirt texture `{key}` ignored: not a readable image");
            }
        }
    }

    /// The dirt texture `vc` lights, when it names one that loaded.
    pub(crate) fn lens_dirt(&self, vc: &PostProcessingComponent) -> Option<Rc<GpuTexture>> {
        self.gpu_textures.get(file(vc)?.as_str()).cloned()
    }
}

/// The 1×1 black texture bound while a view has no dirt (wgpu zero-fills a new
/// texture): the composite's add of `bloom × dirt` is then exactly nothing.
pub(crate) fn black(device: &wgpu::Device) -> wgpu::TextureView {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Bloom No Dirt"),
        size: wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    texture.create_view(&Default::default())
}
