use super::atlas::{GlyphAtlas, WIDTH};
use crate::gpu::ops::{CountedQueue as _, Writes};
use crate::ui::{bind, UiRenderer};

pub(crate) struct AtlasGpu {
    texture: wgpu::Texture,
    pub(crate) group: wgpu::BindGroup,
}

impl UiRenderer {
    pub(crate) fn upload_atlases(&mut self, device: &wgpu::Device, writes: Writes) {
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
                write(writes, &gpu.texture, atlas);
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
        // Distances are data, not colour: no sRGB decode.
        format: wgpu::TextureFormat::R8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    })
}

fn write((queue, staging): Writes, texture: &wgpu::Texture, atlas: &GlyphAtlas) {
    queue.counted_write_texture(
        staging,
        texture.as_image_copy(),
        &atlas.pixels,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(WIDTH),
            rows_per_image: None,
        },
        texture.size(),
    );
}
