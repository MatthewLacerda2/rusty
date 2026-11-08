//! KTX2 cubemap loading for reflection probes (#244).
//!
//! Loads a baked reflection-probe cubemap (a KTX2 file) into a wgpu cube texture the
//! probe array copies in. The container is read here, with `ktx2` naming the formats
//! only: its `Reader` bounds-checks one level of the index and its offset sums can
//! overflow, so a corrupt file would panic it (#1609).
//!
//! A missing, unreadable or odd-format file must NOT panic: it is refused with a
//! reason, and the probe array warns once and leaves it out. Only BC6H unsigned
//! float (`BC6H_UFLOAT_BLOCK`) is accepted, the bake's output format since #1885; a
//! half-float or 8-bit cube baked before then is refused with a warning to re-bake
//! it, never a crash. On an adapter without `TEXTURE_COMPRESSION_BC` the blocks are
//! decoded to `Rgba16Float` on the CPU at load ([`CubemapMips::decoded`]). One rule decides what a cube
//! may be, [`cube_shape`] over the header and [`level_ranges`] over the level index;
//! the header-only read ([`read_cube_shape`]) and the full parse
//! ([`parse_ktx2_cubemap_mips`]) both apply it, as pure functions over bytes so they
//! are unit-testable without a GPU or a file on disk.

use ktx2::Format;

use super::reflection_prefilter::source::encode_face;
use super::reflection_probes::MAX_FACE;
use crate::asset::lightmap_page::bc6h_texels;
use crate::asset::texture_import::blocks_len;

/// An HDR cubemap WITH its roughness mip chain (#245, #1885), in `format`: BC6H
/// blocks as the file holds them, or `Rgba16Float` texels once [decoded](Self::decoded).
/// `mips[0]` is the base level (`size`); each later mip halves the edge length. Every
/// level is the six faces packed back-to-back in KTX2 face order (+X, -X, +Y, -Y, +Z,
/// -Z). This is what the forward pass uploads so `textureSampleLevel` can pick a mip
/// from a surface's roughness.
pub struct CubemapMips {
    pub size: u32,
    pub format: wgpu::TextureFormat,
    pub mips: Vec<Vec<u8>>,
}

/// The format a baked cube's file holds.
pub(crate) const CUBE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bc6hRgbUfloat;
/// The bytes of one BC6H block (4×4 texels).
const BLOCK_BYTES: u32 = 16;

/// Why a cube file is not one the probe array can read, for its one warning (#1609).
pub(crate) type Refusal = &'static str;

/// The KTX2 header's length, where the level index starts, and one index entry's.
const HEADER: usize = 80;
const LEVEL_ENTRY: usize = 24;

impl CubemapMips {
    /// The cube with every face's blocks decoded to `Rgba16Float` texels: the upload
    /// on an adapter without BC compression (#1885). Already decoded, it is returned
    /// as it is.
    pub(crate) fn decoded(self) -> Self {
        if self.format != CUBE_FORMAT {
            return self;
        }
        let mips = (self.mips.iter().enumerate())
            .map(|(level, bytes)| {
                let edge = (self.size >> level).max(1);
                let face = blocks_len(edge, edge, BLOCK_BYTES);
                (bytes.chunks_exact(face))
                    .flat_map(|blocks| encode_face(&bc6h_texels(edge, blocks)))
                    .collect()
            })
            .collect();
        Self {
            format: wgpu::TextureFormat::Rgba16Float,
            mips,
            ..self
        }
    }

    /// The base face size and mip count this cube holds.
    pub(crate) fn shape(&self) -> CubeShape {
        CubeShape {
            size: self.size,
            mips: self.mips.len() as u32,
        }
    }
}

/// Parse a KTX2 blob into a BC6H cubemap WITH its full roughness mip chain (#245),
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
        format: CUBE_FORMAT,
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
/// reads: a `BC6H_UFLOAT_BLOCK` square cube (six faces, no depth, at most one layer)
/// of whole blocks no larger than [`MAX_FACE`], not supercompressed, with no more mip
/// levels than its size has.
pub(crate) fn cube_shape(header: &[u8; 48]) -> Result<CubeShape, Refusal> {
    const MAGIC: [u8; 12] = [
        0xAB, 0x4B, 0x54, 0x58, 0x20, 0x32, 0x30, 0xBB, 0x0D, 0x0A, 0x1A, 0x0A,
    ];
    let word = |at: usize| u32::from_le_bytes(header[at..at + 4].try_into().unwrap());
    let size = word(20);
    let refusal = if header[..12] != MAGIC {
        Some("it is not a KTX2 file")
    } else if Format::new(word(12)).is_some_and(is_rgba8) {
        Some("it is an 8-bit cube from before HDR probes; re-bake it")
    } else if Format::new(word(12)) == Some(Format::R16G16B16A16_SFLOAT) {
        Some("it is an uncompressed cube from before BC6H probes; re-bake it")
    } else if Format::new(word(12)) != Some(Format::BC6H_UFLOAT_BLOCK) {
        Some("it is not BC6H")
    } else if size == 0 || word(24) != size || word(28) != 0 {
        Some("its faces are not square")
    } else if !size.is_multiple_of(4) {
        Some("its faces are not whole 4×4 blocks")
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
/// its six faces' blocks (a level under 4×4 holds one padded block a face). A
/// level's bytes past its faces are not read.
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
        let edge = u64::from((shape.size >> i).max(1)).div_ceil(4);
        let faces = (edge.checked_mul(edge * u64::from(BLOCK_BYTES) * 6))
            .ok_or("its faces are too large")?;
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

/// True for the 8-bit RGBA formats reflection bakes wrote before #1696 (UNORM or
/// SRGB): refused with their own reason, so the warning says to re-bake.
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

#[cfg(test)]
#[path = "cubemap_tests.rs"]
mod cubemap_tests;

#[cfg(test)]
#[path = "cube_shape_tests.rs"]
mod cube_shape_tests;

#[cfg(test)]
#[path = "corrupt_cube_tests.rs"]
mod corrupt_cube_tests;
