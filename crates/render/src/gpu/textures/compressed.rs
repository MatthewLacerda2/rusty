//! Uploads the import cache's BC levels as they are: a BC texture cannot be rendered into,
//! so no mip is drawn, and the renderer never encodes one.

use std::collections::BTreeSet;
use std::sync::{Mutex, Once};

use crate::gpu::ops::CountedQueue as _;
use rusty_asset::sidecar::{Compression, TextureSettings};
use rusty_asset::texture_import::{self, BlockFormat, Compressed, TextureKind};

/// What a compressed upload needs; without it every texture uploads as RGBA8.
pub(crate) const FEATURE: wgpu::Features = wgpu::Features::TEXTURE_COMPRESSION_BC;

pub(crate) fn formats(
    format: BlockFormat,
    srgb: bool,
) -> (wgpu::TextureFormat, wgpu::TextureFormat) {
    use wgpu::TextureFormat as F;
    match format {
        BlockFormat::Bc7 if srgb => (F::Bc7RgbaUnormSrgb, F::Bc7RgbaUnorm),
        BlockFormat::Bc7 => (F::Bc7RgbaUnorm, F::Bc7RgbaUnorm),
        BlockFormat::Bc4 => (F::Bc4RUnorm, F::Bc4RUnorm),
        BlockFormat::Bc5 => (F::Bc5RgUnorm, F::Bc5RgUnorm),
    }
}

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

pub(crate) fn reads_as(format: wgpu::TextureFormat, kind: TextureKind) -> bool {
    use wgpu::TextureFormat as F;
    match format {
        F::Bc5RgUnorm => kind == TextureKind::Normal,
        F::Bc4RUnorm => kind == TextureKind::Mask,
        super::deep::DEEP_FORMAT => kind == TextureKind::Normal,
        _ => true,
    }
}

pub(super) fn upload(
    (device, (queue, staging)): (&wgpu::Device, crate::gpu::ops::Writes),
    label: &str,
    texture: &Compressed,
    srgb: bool,
) -> (wgpu::Texture, wgpu::TextureFormat) {
    let (format, data_format) = formats(texture.format, srgb);
    let size = super::extent(texture.width, texture.height);
    let mut usage = wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST;
    // lint: allow(cfg-test-switch) only render's own unit tests read a loaded texture back
    if cfg!(test) {
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
        queue.counted_write_texture(
            staging,
            wgpu::TexelCopyTextureInfo {
                texture: &gpu,
                mip_level: mip as u32,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            blocks,
            // BC blocks are 4×4 texels.
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
