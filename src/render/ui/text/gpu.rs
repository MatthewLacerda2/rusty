//! src/render/ui/text/gpu.rs — the atlases' GPU copies (#419).
//!
//! Each font atlas is an `R8Unorm` texture (distances are data, not colour, so no
//! sRGB decode), re-uploaded whole when its CPU side is dirty and recreated when
//! it has grown. A static HUD therefore uploads nothing per frame; a new glyph
//! costs one atlas upload.

use super::atlas::{GlyphAtlas, WIDTH};
use crate::render::ui::{bind, UiRenderer};

/// One atlas on the GPU.
pub(crate) struct AtlasGpu {
    texture: wgpu::Texture,
    pub(crate) group: wgpu::BindGroup,
}

impl UiRenderer {
    /// Upload every dirty atlas.
    pub(crate) fn upload_atlases(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        for (path, atlas) in self.atlases.atlases.iter_mut().filter(|(_, a)| a.dirty) {
            let fits = self
                .fonts
                .get(path)
                .is_some_and(|g| g.texture.height() == atlas.height);
            if !fits {
                let texture = create(device, atlas.height);
                let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
                let group = bind(device, &self.layout, &view, &self.sampler);
                self.fonts.insert(path.clone(), AtlasGpu { texture, group });
            }
            if let Some(gpu) = self.fonts.get(path) {
                write(queue, &gpu.texture, atlas);
            }
            atlas.dirty = false;
        }
    }
}

fn create(device: &wgpu::Device, height: u32) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("UI Glyph Atlas"),
        size: wgpu::Extent3d {
            width: WIDTH,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::R8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    })
}

fn write(queue: &wgpu::Queue, texture: &wgpu::Texture, atlas: &GlyphAtlas) {
    queue.write_texture(
        texture.as_image_copy(),
        &atlas.pixels,
        wgpu::ImageDataLayout {
            offset: 0,
            bytes_per_row: Some(WIDTH),
            rows_per_image: None,
        },
        texture.size(),
    );
}
