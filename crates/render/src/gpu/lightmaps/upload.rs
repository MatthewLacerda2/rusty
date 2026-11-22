use super::load::Loaded;
use crate::gpu::ops::{CountedQueue as _, Writes};

pub(crate) struct LightmapArray {
    pub shape: (u32, u32),
    pub bytes: u64,
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
}

/// Bytes (1 GiB): wgpu's limits name no texture-memory bound. A Dust2 directional bake stays
/// under it at 8 texels a unit.
pub(crate) const MAX_ARRAY_BYTES: u64 = 1 << 30;

/// Past the limits, `create_texture` would panic in wgpu's default error handler on every load.
pub(crate) fn fits(
    limits: &wgpu::Limits,
    edge: u32,
    layers: usize,
    bytes: u64,
) -> Result<(), String> {
    if bytes > MAX_ARRAY_BYTES {
        return Err(format!(
            "{layers} pages taking {} MiB, more than the {} MiB a lightmap array may; \
             rebake with fewer texels a unit",
            bytes >> 20,
            MAX_ARRAY_BYTES >> 20
        ));
    }
    if layers > limits.max_texture_array_layers as usize {
        return Err(format!(
            "{layers} pages, more than the {} layers a texture array holds on this GPU; \
             rebake with a larger Max Lightmap Size",
            limits.max_texture_array_layers
        ));
    }
    if edge > limits.max_texture_dimension_2d {
        return Err(format!(
            "pages of {edge}², larger than the {}² this GPU takes; rebake with a smaller \
             Max Lightmap Size",
            limits.max_texture_dimension_2d
        ));
    }
    Ok(())
}

pub(crate) fn upload(
    device: &wgpu::Device,
    (queue, staging): Writes,
    loaded: &Loaded,
) -> LightmapArray {
    let levels = loaded.layers[0].len() as u32;
    let array = create(
        device,
        loaded.format,
        loaded.edge,
        loaded.layers.len() as u32,
        levels,
    );
    let (texture, size) = (&array.texture, array.texture.size());
    let (bw, bh) = loaded.format.block_dimensions();
    let block = loaded.format.block_copy_size(None).unwrap_or(4);
    for (layer, chain) in loaded.layers.iter().enumerate() {
        for (mip, bytes) in chain.iter().enumerate() {
            let level = size.mip_level_size(mip as u32, wgpu::TextureDimension::D2);
            let physical = wgpu::Extent3d {
                depth_or_array_layers: 1,
                ..level.physical_size(loaded.format)
            };
            queue.counted_write_texture(
                staging,
                wgpu::TexelCopyTextureInfo {
                    texture,
                    mip_level: mip as u32,
                    origin: wgpu::Origin3d {
                        x: 0,
                        y: 0,
                        z: layer as u32,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                bytes,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(physical.width / bw * block),
                    rows_per_image: Some(physical.height / bh),
                },
                physical,
            );
        }
    }
    LightmapArray {
        bytes: loaded.bytes(),
        ..array
    }
}

pub(crate) fn create(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    edge: u32,
    layers: u32,
    levels: u32,
) -> LightmapArray {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Lightmap Pages"),
        size: wgpu::Extent3d {
            width: edge,
            height: edge,
            depth_or_array_layers: layers,
        },
        mip_level_count: levels,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor {
        label: Some("Lightmap Pages"),
        dimension: Some(wgpu::TextureViewDimension::D2Array),
        ..Default::default()
    });
    LightmapArray {
        shape: (edge, layers),
        bytes: 0,
        texture,
        view,
    }
}

impl crate::Renderer {
    /// A test hook: `dev::memory`'s lightmap test measures what one upload holds.
    #[doc(hidden)]
    pub fn upload_lightmap_pages(
        &self,
        edge: u32,
        pages: Vec<Vec<u8>>,
    ) -> (wgpu::Texture, Vec<Vec<u8>>) {
        let loaded = Loaded {
            format: wgpu::TextureFormat::Rgba16Float,
            edge,
            layers: pages.into_iter().map(|page| vec![page]).collect(),
        };
        let writes = (&self.queue, &self.staging);
        let texture = upload(&self.device, writes, &loaded).texture;
        (texture, loaded.layers.into_iter().flatten().collect())
    }
}
