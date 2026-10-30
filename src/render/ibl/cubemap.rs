//! KTX2 cubemap loading for reflection probes (#244).
//!
//! Loads a baked reflection-probe cubemap (a KTX2 file) into a wgpu cube texture the
//! probe array copies in. The container is read here, with `ktx2` naming the formats
//! only: its `Reader` bounds-checks one level of the index and its offset sums can
//! overflow, so a corrupt file would panic it (#1609).
//!
//! A missing, unreadable or odd-format file must NOT panic: it is refused with a
//! reason, and the probe array warns once and leaves it out. Only LDR RGBA8 (UNORM /
//! SRGB) is accepted, matching the bake's output format. One rule decides what a cube
//! may be, [`cube_shape`] over the header and [`level_ranges`] over the level index;
//! the header-only read ([`read_cube_shape`]) and the full parse
//! ([`parse_ktx2_cubemap_mips`]) both apply it, as pure functions over bytes so they
//! are unit-testable without a GPU or a file on disk.

use ktx2::Format;

use super::reflection_probes::MAX_FACE;
use crate::render::Renderer;

/// A decoded LDR cubemap WITH its roughness mip chain (#245). `mips[0]` is the base level
/// (`size`); each later mip halves the edge length. Every level is the six faces packed
/// back-to-back in KTX2 face order (+X, -X, +Y, -Y, +Z, -Z). This is what the forward
/// pass uploads so `textureSampleLevel` can pick a mip from a surface's roughness.
pub struct CubemapMips {
    pub size: u32,
    pub mips: Vec<Vec<u8>>,
}

/// Why a cube file is not one the probe array can read, for its one warning (#1609).
pub(crate) type Refusal = &'static str;

/// The KTX2 header's length, where the level index starts, and one index entry's.
const HEADER: usize = 80;
const LEVEL_ENTRY: usize = 24;

impl CubemapMips {
    /// The base face size and mip count this cube holds.
    pub(crate) fn shape(&self) -> CubeShape {
        CubeShape {
            size: self.size,
            mips: self.mips.len() as u32,
        }
    }
}

/// Parse a KTX2 blob into an LDR RGBA8 cubemap WITH its full roughness mip chain (#245),
/// or why it is not one. Level 0 is the base; each later level halves the edge length
/// (the prefilter's layout). The file must be what [`cube_shape`] and [`level_ranges`]
/// accept, the rule [`read_cube_shape`] judges a header by, so the two never disagree
/// and no byte sequence panics (#1609). Pure and GPU-free so it is testable headlessly.
pub fn parse_ktx2_cubemap_mips(bytes: &[u8]) -> Result<CubemapMips, Refusal> {
    let header = bytes
        .get(..HEADER)
        .ok_or("it is shorter than a KTX2 header")?;
    let shape = cube_shape(header[..48].try_into().unwrap())?;
    let ranges = level_ranges(&bytes[HEADER..], shape, bytes.len() as u64)?;
    let mips = ranges.into_iter().map(|r| bytes[r].to_vec()).collect();
    Ok(CubemapMips {
        size: shape.size,
        mips,
    })
}

/// A cube file's base face size and mip count, as its KTX2 header states them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CubeShape {
    pub size: u32,
    pub mips: u32,
}

/// The shape of the cube at `path`, read from its header and level index alone
/// (#1584): the probe array budgets a cube before it decides to load it, so a cube it
/// passes over costs a few hundred bytes of disk, not a decode. Refused when the file
/// is missing or is not one [`parse_ktx2_cubemap_mips`] would accept (#1609).
pub(crate) fn read_cube_shape(path: &str) -> Result<CubeShape, Refusal> {
    use std::io::Read;
    let mut file = std::fs::File::open(path).map_err(|_| "it cannot be opened")?;
    let len = file.metadata().map_err(|_| "it cannot be opened")?.len();
    let mut header = [0u8; HEADER];
    (file.read_exact(&mut header)).map_err(|_| "it is shorter than a KTX2 header")?;
    let shape = cube_shape(header[..48].try_into().unwrap())?;
    let mut index = vec![0u8; shape.mips as usize * LEVEL_ENTRY];
    (file.read_exact(&mut index)).map_err(|_| "its level index is cut short")?;
    level_ranges(&index, shape, len)?;
    Ok(shape)
}

/// The shape a KTX2 header's first 48 bytes state, when it is a cube the loader
/// reads: an LDR RGBA8 square cube (six faces, no depth, at most one layer) no
/// larger than [`MAX_FACE`], not supercompressed, with no more mip levels than its
/// size has.
pub(crate) fn cube_shape(header: &[u8; 48]) -> Result<CubeShape, Refusal> {
    const MAGIC: [u8; 12] = [
        0xAB, 0x4B, 0x54, 0x58, 0x20, 0x32, 0x30, 0xBB, 0x0D, 0x0A, 0x1A, 0x0A,
    ];
    let word = |at: usize| u32::from_le_bytes(header[at..at + 4].try_into().unwrap());
    let size = word(20);
    let refusal = if header[..12] != MAGIC {
        Some("it is not a KTX2 file")
    } else if !Format::new(word(12)).is_some_and(is_rgba8) {
        Some("it is not RGBA8")
    } else if size == 0 || word(24) != size || word(28) != 0 {
        Some("its faces are not square")
    } else if size > MAX_FACE {
        Some("its faces are larger than a probe bakes")
    } else if word(32) > 1 || word(36) != 6 {
        Some("it is not one cube of six faces")
    } else if word(44) != 0 {
        Some("it is supercompressed")
    } else if word(40) > u32::BITS - size.leading_zeros() {
        Some("it states more mip levels than its size has")
    } else {
        None
    };
    refusal.map_or(
        Ok(CubeShape {
            size,
            mips: word(40).max(1),
        }),
        Err,
    )
}

/// The bytes of each of `shape`'s levels, from its level `index` (what follows the
/// header) in a file of `len` bytes: every level must lie inside the file and hold
/// its six faces. A level's bytes past its faces are not read.
fn level_ranges(
    index: &[u8],
    shape: CubeShape,
    len: u64,
) -> Result<Vec<std::ops::Range<usize>>, Refusal> {
    let entries =
        (index.get(..shape.mips as usize * LEVEL_ENTRY)).ok_or("its level index is cut short")?;
    let level = |(i, entry): (usize, &[u8])| {
        let at = |o: usize| u64::from_le_bytes(entry[o..o + 8].try_into().unwrap());
        let (offset, length) = (at(0), at(8));
        let edge = u64::from((shape.size >> i).max(1));
        let faces = (edge.checked_mul(edge * 4 * 6)).ok_or("its faces are too large")?;
        let end = (offset.checked_add(faces)).filter(|&end| length >= faces && end <= len);
        let end = end.ok_or("a level lies past its end or is short of its faces")?;
        Ok(usize::try_from(offset).map_err(|_| "it is too large")?..end as usize)
    };
    entries
        .chunks_exact(LEVEL_ENTRY)
        .enumerate()
        .map(level)
        .collect()
}

/// True for the LDR RGBA8 formats a reflection bake emits (UNORM or SRGB).
fn is_rgba8(format: Format) -> bool {
    matches!(format, Format::R8G8B8A8_UNORM | Format::R8G8B8A8_SRGB)
}

/// A reflection-probe cubemap resident on the GPU: a 6-layer cube texture + its cube
/// view + a sampler. Deliberately NOT a `GpuTexture` (that type's bind group targets the
/// 2D material layout). The forward pass never binds it directly: the probe array
/// copies it into a layer of its own (#1206).
pub struct CubemapTexture {
    pub texture: wgpu::Texture,
    pub size: u32,
}

impl Renderer {
    /// Load a reflection-probe cubemap WITH its roughness mip chain from `path` (#245),
    /// into a mipped cube texture the probe array copies in. Refused when the file is
    /// missing, is not a usable LDR RGBA8 cubemap, or is not the `expect`ed shape (size
    /// and mip count) the frame planned for, which is decoded but never uploaded
    /// (#1245, #1609).
    pub(crate) fn load_cubemap_mips(
        &self,
        path: &str,
        expect: CubeShape,
    ) -> Result<CubemapTexture, Refusal> {
        let bytes = std::fs::read(path).map_err(|_| "it cannot be read")?;
        let data = parse_ktx2_cubemap_mips(&bytes)?;
        if data.shape() != expect {
            return Err("it changed shape since its header was read");
        }
        Ok(self.upload_cubemap_mips(path, &data))
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
            // COPY_SRC: the probe array copies it into its layer (#1206).
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        for (level, bytes) in data.mips.iter().enumerate() {
            let level_size = (data.size >> level).max(1);
            self.write_cube_level(&texture, level as u32, level_size, bytes);
        }
        Self::finish_cube_texture(texture, data.size)
    }

    /// Write one mip level's six faces into a cube texture.
    fn write_cube_level(&self, texture: &wgpu::Texture, level: u32, size: u32, bytes: &[u8]) {
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: level,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            bytes,
            wgpu::TexelCopyBufferLayout {
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

    /// Wrap an uploaded cube texture.
    fn finish_cube_texture(texture: wgpu::Texture, size: u32) -> CubemapTexture {
        CubemapTexture { texture, size }
    }
}

#[cfg(test)]
#[path = "cubemap_tests.rs"]
mod cubemap_tests;

#[cfg(test)]
#[path = "cube_shape_tests.rs"]
mod cube_shape_tests;

#[cfg(test)]
#[path = "corrupt_cube_tests.rs"]
mod corrupt_cube_tests;
