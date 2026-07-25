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

use std::rc::Rc;

use ktx2::Format;

use crate::render::Renderer;

/// A decoded LDR cubemap on the CPU: a square edge length plus the six faces' RGBA8
/// pixels in KTX2 face order (+X, -X, +Y, -Y, +Z, -Z), each `size * size * 4` bytes.
pub struct CubemapData {
    pub size: u32,
    /// The six faces concatenated in face order — `6 * size * size * 4` bytes total,
    /// exactly the layout a `depth_or_array_layers = 6` upload expects.
    pub faces_rgba8: Vec<u8>,
}

/// A decoded LDR cubemap WITH its roughness mip chain (#245). `mips[0]` is the base level
/// (`size`); each later mip halves the edge length. Every level is the six faces packed
/// back-to-back, the same layout `CubemapData::faces_rgba8` uses. This is what the forward
/// pass uploads so `textureSampleLevel` can pick a mip from a surface's roughness.
pub struct CubemapMips {
    pub size: u32,
    pub mips: Vec<Vec<u8>>,
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

/// Parse a KTX2 blob into an LDR RGBA8 cubemap WITH its full roughness mip chain (#245),
/// or `None` when it is not a usable LDR cubemap. Level 0 is the base; each later level is
/// expected to halve the edge length (the prefilter's layout). A level whose byte length
/// is short for its expected size truncates the chain there rather than failing — the
/// loader stays tolerant, matching `parse_ktx2_cubemap`'s graceful-absence contract.
pub fn parse_ktx2_cubemap_mips(bytes: &[u8]) -> Option<CubemapMips> {
    let reader = ktx2::Reader::new(bytes).ok()?;
    let header = reader.header();
    if header.face_count != 6 || header.layer_count > 1 || !is_rgba8(header.format?) {
        return None;
    }
    let size = header.pixel_width;
    if size == 0 || header.pixel_height != size {
        return None;
    }

    let mut mips = Vec::new();
    for (level, data) in reader.levels().enumerate() {
        let level_size = (size >> level).max(1);
        let expected = (level_size as usize) * (level_size as usize) * 4 * 6;
        if data.len() < expected {
            break;
        }
        mips.push(data[..expected].to_vec());
    }
    if mips.is_empty() {
        return None;
    }
    Some(CubemapMips { size, mips })
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

    /// Load a reflection-probe cubemap WITH its roughness mip chain from `path` (#245),
    /// into a mipped cube texture the forward pass samples with `textureSampleLevel`.
    /// Returns `None` (caller falls back to the skybox) when the file is missing or is not
    /// a usable LDR RGBA8 cubemap — the graceful-absence contract of `load_cubemap`.
    pub fn load_cubemap_mips(&self, path: &str) -> Option<CubemapTexture> {
        let bytes = std::fs::read(path).ok()?;
        let data = parse_ktx2_cubemap_mips(&bytes)?;
        Some(self.upload_cubemap_mips(path, &data))
    }

    /// [`load_cubemap_mips`](Self::load_cubemap_mips) behind a by-path content cache
    /// (#355), returning a shared handle.
    ///
    /// Which cube is *bound* is per-scene state: two scenes with different reflection
    /// probes rendering in one frame legitimately swap it every frame. Uncached, each
    /// swap re-read the KTX2 from disk and re-uploaded every mip — a full disk load
    /// per frame, per flip. The failure is cached too, so an unreadable path is not
    /// retried on every flip either.
    pub fn cached_cubemap(&mut self, path: &str) -> Option<Rc<CubemapTexture>> {
        if let Some(hit) = self.cubemaps.get(path) {
            return hit.clone();
        }
        let loaded = self.load_cubemap_mips(path).map(Rc::new);
        self.cubemaps.insert(path.to_string(), loaded.clone());
        loaded
    }

    /// Upload a decoded mipped cubemap into a mipped 6-layer cube texture with a cube view
    /// + trilinear sampler. Each mip's six faces upload at their own level/size.
    fn upload_cubemap_mips(&self, label: &str, data: &CubemapMips) -> CubemapTexture {
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: data.size,
                height: data.size,
                depth_or_array_layers: 6,
            },
            mip_level_count: data.mips.len() as u32,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        for (level, bytes) in data.mips.iter().enumerate() {
            let level_size = (data.size >> level).max(1);
            self.write_cube_level(&texture, level as u32, level_size, bytes);
        }
        self.finish_cube_texture(label, texture, data.size)
    }

    /// Write one mip level's six faces into a cube texture.
    fn write_cube_level(&self, texture: &wgpu::Texture, level: u32, size: u32, bytes: &[u8]) {
        self.queue.write_texture(
            wgpu::ImageCopyTexture {
                texture,
                mip_level: level,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            bytes,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4 * size),
                rows_per_image: Some(size),
            },
            wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 6,
            },
        );
    }

    /// Build the cube view + trilinear sampler wrapping an uploaded cube texture.
    fn finish_cube_texture(
        &self,
        label: &str,
        texture: wgpu::Texture,
        size: u32,
    ) -> CubemapTexture {
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
            size,
        }
    }

    /// Upload a decoded single-level cubemap into a 6-layer cube texture with a cube view
    /// + sampler.
    fn upload_cubemap(&self, label: &str, data: &CubemapData) -> CubemapTexture {
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: data.size,
                height: data.size,
                depth_or_array_layers: 6,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.write_cube_level(&texture, 0, data.size, &data.faces_rgba8);
        self.finish_cube_texture(label, texture, data.size)
    }
}

/// A 1×1×6 black fallback cube, always available so the group-0 bind group satisfies the
/// cube-texture binding even when no reflection probe is active (#245). The shader ignores
/// it via the `refl_has_cubemap` flag and samples the 2D skybox instead. Built from a raw
/// device so it can be constructed during renderer assembly before `self` exists.
pub fn fallback_cube(device: &wgpu::Device) -> CubemapTexture {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Fallback Reflection Cube"),
        size: wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 6,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor {
        label: Some("Fallback Reflection Cube"),
        dimension: Some(wgpu::TextureViewDimension::Cube),
        ..Default::default()
    });
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor::default());
    CubemapTexture {
        texture,
        view,
        sampler,
        size: 1,
    }
}

#[cfg(test)]
#[path = "cubemap_tests.rs"]
mod cubemap_tests;
