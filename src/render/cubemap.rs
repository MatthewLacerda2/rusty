//! src/render/cubemap.rs — KTX2 cubemap loading for reflection probes (#244).
//!
//! Loads a baked reflection-probe cubemap (a KTX2 file) into a wgpu cube texture,
//! paralleling the skybox `load_texture` path. The KTX2 parse is `ktx2` (parse-only);
//! the decoded faces upload into a `D2Array`-of-6 cube texture.
//!
//! Graceful absence is a first-class requirement: there are NO real baked cubemaps
//! until the bake lands (#245), so a missing/unreadable/odd-format file must NOT panic
//! or error — [`load_ktx2_cubemap`] returns `None` and the caller falls back to the
//! global skybox. Only LDR RGBA8 (UNORM / SRGB) is accepted, matching the bake's output
//! format; anything else is treated as absent.
//!
//! The CPU-side parse ([`parse_ktx2_cubemap`]) is split out as a pure function over
//! bytes so it is unit-testable without a GPU or a file on disk.

use ktx2::Format;

use super::Renderer;

/// A decoded LDR cubemap on the CPU: a square edge length plus the six faces' RGBA8
/// pixels in KTX2 face order (+X, -X, +Y, -Y, +Z, -Z), each `size * size * 4` bytes.
pub struct CubemapData {
    pub size: u32,
    /// The six faces concatenated in face order — `6 * size * size * 4` bytes total,
    /// exactly the layout a `depth_or_array_layers = 6` upload expects.
    pub faces_rgba8: Vec<u8>,
}

/// Parse a KTX2 byte blob into an LDR RGBA8 cubemap, or `None` when the bytes are not a
/// usable LDR cubemap (bad/garbage data, non-cube, non-RGBA8, or a size mismatch). Pure
/// and GPU-free so the loading path is testable headlessly.
pub fn parse_ktx2_cubemap(bytes: &[u8]) -> Option<CubemapData> {
    let reader = ktx2::Reader::new(bytes).ok()?;
    let header = reader.header();

    // Must be a square cubemap (6 faces, single array layer) in an RGBA8 LDR format.
    if header.face_count != 6 || header.layer_count > 1 {
        return None;
    }
    if !is_rgba8(header.format?) {
        return None;
    }
    let size = header.pixel_width;
    if size == 0 || header.pixel_height != size {
        return None;
    }

    // Level 0 holds all six faces packed back-to-back; later mips (the prefilter chain,
    // #245) are ignored for now — we sample the base level until roughness mips exist.
    let level0 = reader.levels().next()?;
    let face_bytes = (size as usize) * (size as usize) * 4;
    let expected = face_bytes * 6;
    if level0.len() < expected {
        return None;
    }

    Some(CubemapData {
        size,
        faces_rgba8: level0[..expected].to_vec(),
    })
}

/// True for the LDR RGBA8 formats a reflection bake emits (UNORM or SRGB).
fn is_rgba8(format: Format) -> bool {
    matches!(format, Format::R8G8B8A8_UNORM | Format::R8G8B8A8_SRGB)
}

/// A reflection-probe cubemap resident on the GPU: a 6-layer cube texture + its cube
/// view + a sampler. Deliberately NOT a `GpuTexture` (that type's bind group targets the
/// 2D material layout); the forward pipeline's cube binding + per-probe selection land
/// with the bake (#245), at which point this view feeds it.
pub struct CubemapTexture {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
    pub size: u32,
}

impl Renderer {
    /// Load a reflection-probe cubemap from `path` into a GPU cube texture. Returns
    /// `None` (caller falls back to the skybox) when the file is missing or is not a
    /// usable LDR RGBA8 cubemap — the expected state until the bake exists (#245).
    pub fn load_cubemap(&self, path: &str) -> Option<CubemapTexture> {
        let bytes = std::fs::read(path).ok()?;
        let data = parse_ktx2_cubemap(&bytes)?;
        Some(self.upload_cubemap(path, &data))
    }

    /// Upload a decoded cubemap into a 6-layer cube texture with a cube view + sampler.
    fn upload_cubemap(&self, label: &str, data: &CubemapData) -> CubemapTexture {
        let size = wgpu::Extent3d {
            width: data.size,
            height: data.size,
            depth_or_array_layers: 6,
        };
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &data.faces_rgba8,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4 * data.size),
                rows_per_image: Some(data.size),
            },
            size,
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some(label),
            dimension: Some(wgpu::TextureViewDimension::Cube),
            ..Default::default()
        });
        let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        CubemapTexture {
            texture,
            view,
            sampler,
            size: data.size,
        }
    }
}

#[cfg(test)]
#[path = "cubemap_tests.rs"]
mod cubemap_tests;
