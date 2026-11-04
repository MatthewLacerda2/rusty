//! Creating a lightmap page array and writing its levels (#438, #1694).

use super::load::Loaded;

/// One uploaded page array.
pub(crate) struct LightmapArray {
    /// Page edge and page count, to match direction pages to colour pages.
    pub shape: (u32, u32),
    /// The bytes it takes on the GPU, every level of every page.
    pub bytes: u64,
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
}

/// Whether `limits` take a page list of `layers` pages `edge` texels a side as one
/// array (#1704). A file baked past the bake's [`MAX_PAGES`] refusal (a hand edit,
/// another engine version) would panic `create_texture` in wgpu's default error
/// handler on every load; `Err` instead leaves the scene unlit by lightmaps.
///
/// [`MAX_PAGES`]: crate::scene::lighting::lightmap::MAX_PAGES
pub(crate) fn fits(limits: &wgpu::Limits, edge: u32, layers: usize) -> Result<(), String> {
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

/// Upload `loaded` as a 2D texture array with its mip chain.
pub(crate) fn upload(device: &wgpu::Device, queue: &wgpu::Queue, loaded: &Loaded) -> LightmapArray {
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
            queue.write_texture(
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
    let bytes = loaded.layers.iter().flatten().map(|l| l.len() as u64).sum();
    LightmapArray { bytes, ..array }
}

/// A zeroed `edge`² array of `layers` pages and `levels` mips in `format`, its
/// view and its clamped trilinear sampler.
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
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("Lightmap Sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::MipmapFilterMode::Linear,
        ..Default::default()
    });
    LightmapArray {
        shape: (edge, layers),
        bytes: 0,
        texture,
        view,
        sampler,
    }
}
