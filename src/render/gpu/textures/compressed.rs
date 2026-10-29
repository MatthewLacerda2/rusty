//! Uploading a block-compressed texture (#1574): the levels the import cache holds
//! (`asset::texture_import`), written as they are, so no mip is drawn on the GPU
//! (a BC texture cannot be rendered into). Colour formats keep the sRGB storage
//! and raw data view every image upload has (#647). The renderer never encodes:
//! a texture without a current cache uploads uncompressed until the import makes
//! one.

use std::collections::BTreeSet;
use std::sync::{Mutex, Once};

use crate::asset::sidecar::{Compression, TextureSettings};
use crate::asset::texture_import::{self, BlockFormat, Compressed, TextureKind};

/// The device feature a compressed upload needs.
pub(crate) const FEATURE: wgpu::Features = wgpu::Features::TEXTURE_COMPRESSION_BC;

/// The format a texture of `format` is stored as, and the one its data view reads
/// the same blocks as.
pub(crate) fn formats(format: BlockFormat) -> (wgpu::TextureFormat, wgpu::TextureFormat) {
    use wgpu::TextureFormat as F;
    match format {
        BlockFormat::Bc7 => (F::Bc7RgbaUnormSrgb, F::Bc7RgbaUnorm),
        BlockFormat::Bc4 => (F::Bc4RUnorm, F::Bc4RUnorm),
        BlockFormat::Bc5 => (F::Bc5RgUnorm, F::Bc5RgUnorm),
    }
}

/// Whether a texture with `import` settings is uploaded compressed on a device
/// with `features`. Without BC, one warning per process, then RGBA8.
pub(crate) fn wanted(features: wgpu::Features, import: &TextureSettings) -> bool {
    if import.compression == Compression::None {
        return false;
    }
    if features.contains(FEATURE) {
        return true;
    }
    static WARNED: Once = Once::new();
    WARNED.call_once(|| {
        log::warn!(
            "[Render] this GPU has no BC texture compression: textures upload \
             uncompressed (RGBA8), at four bytes a texel"
        );
    });
    false
}

/// Whether a texture with `import` settings, drawn in a slot holding `kind`, is
/// uploaded compressed on a device with `features`: [`wanted`], and its type fits
/// the slot (`texture_import::fits`), else it would lose channels there.
/// A misfit is warned of once per `path`.
pub(super) fn wanted_for(
    path: &str,
    features: wgpu::Features,
    import: &TextureSettings,
    kind: TextureKind,
) -> bool {
    if texture_import::fits(import.texture_type, kind) {
        return wanted(features, import);
    }
    if import.compression != Compression::None {
        warn_misfit(path, import, kind);
    }
    false
}

/// Paths whose Texture Type has been warned of as not fitting a slot.
static MISFITS: Mutex<BTreeSet<String>> = Mutex::new(BTreeSet::new());

fn warn_misfit(path: &str, import: &TextureSettings, kind: TextureKind) {
    let mut warned = MISFITS.lock().unwrap_or_else(|e| e.into_inner());
    if warned.insert(path.to_owned()) {
        log::warn!(
            "[Render] {path} is a {} texture drawn as {}: uploaded uncompressed for that \
             use; set its Texture Type to fit",
            import.texture_type.name(),
            kind.name(),
        );
    }
}

/// Whether the upload `texture` reads right in a slot holding `kind`: an
/// uncompressed or BC7 one everywhere, BC5 only as a normal map, BC4 only as a mask.
pub(crate) fn reads_as(texture: &wgpu::Texture, kind: TextureKind) -> bool {
    use wgpu::TextureFormat as F;
    match texture.format() {
        F::Bc5RgUnorm => kind == TextureKind::Normal,
        F::Bc4RUnorm => kind == TextureKind::Mask,
        _ => true,
    }
}

/// Create a texture holding `texture`'s levels; returns it with its data view's
/// format.
pub(super) fn upload(
    (device, queue): (&wgpu::Device, &wgpu::Queue),
    label: &str,
    texture: &Compressed,
) -> (wgpu::Texture, wgpu::TextureFormat) {
    let (format, data_format) = formats(texture.format);
    let size = super::extent(texture.width, texture.height);
    let mut usage = wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST;
    if cfg!(test) {
        // Tests read the blocks back.
        usage |= wgpu::TextureUsages::COPY_SRC;
    }
    let view_formats = [data_format];
    let gpu = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size,
        mip_level_count: texture.levels.len() as u32,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage,
        view_formats: match data_format == format {
            true => &[],
            false => &view_formats,
        },
    });
    for (mip, blocks) in texture.levels.iter().enumerate() {
        let level = size.mip_level_size(mip as u32, wgpu::TextureDimension::D2);
        let physical = level.physical_size(format);
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &gpu,
                mip_level: mip as u32,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            blocks,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(physical.width / 4 * texture.format.block_bytes()),
                rows_per_image: Some(physical.height / 4),
            },
            physical,
        );
    }
    (gpu, data_format)
}

/// The bytes `texture` takes on the GPU: every level, in whole blocks for a
/// compressed format.
pub(crate) fn gpu_bytes(texture: &wgpu::Texture) -> u64 {
    let format = texture.format();
    let (bw, bh) = format.block_dimensions();
    let block = u64::from(format.block_copy_size(None).unwrap_or(4));
    let size = texture.size();
    (0..texture.mip_level_count())
        .map(|mip| {
            let level = size.mip_level_size(mip, texture.dimension());
            let p = level.physical_size(format);
            let blocks = u64::from(p.width / bw) * u64::from(p.height / bh);
            blocks * block * u64::from(p.depth_or_array_layers)
        })
        .sum()
}

/// Whether `texture` holds one channel (BC4): the shader reads its red channel
/// where it would read a packed map's own (#1574).
pub(crate) fn is_single_channel(texture: &wgpu::Texture) -> bool {
    texture.format() == formats(BlockFormat::Bc4).0
}
