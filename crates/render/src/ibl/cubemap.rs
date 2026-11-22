//! Parsed here, not by `ktx2::Reader`, whose offset sums overflow and panic on a corrupt file.
//! A missing, corrupt or non-BC6H cube is refused with a reason.

use ktx2::Format;

use super::reflection_prefilter::source::encode_face;
use super::reflection_probes::MAX_FACE;
use rusty_asset::lightmap_page::bc6h_texels;
use rusty_asset::texture_import::blocks_len;

/// Each level packs six faces back-to-back in KTX2 order (+X, -X, +Y, -Y, +Z, -Z).
pub struct CubemapMips {
    pub size: u32,
    pub format: wgpu::TextureFormat,
    pub mips: Vec<Vec<u8>>,
}

pub(crate) const CUBE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bc6hRgbUfloat;
/// Bytes of one 4×4 BC6H block.
const BLOCK_BYTES: u32 = 16;

pub(crate) type Refusal = &'static str;

/// Bytes: the KTX2 header and one level-index entry.
const HEADER: usize = 80;
const LEVEL_ENTRY: usize = 24;

impl CubemapMips {
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

    pub(crate) fn shape(&self) -> CubeShape {
        CubeShape {
            size: self.size,
            mips: self.mips.len() as u32,
        }
    }
}

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CubeShape {
    pub size: u32,
    pub mips: u32,
}

/// Header and level index only, so the probe array budgets a cube without decoding it.
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

/// A level under 4×4 holds one padded block per face.
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

fn is_rgba8(format: Format) -> bool {
    matches!(format, Format::R8G8B8A8_UNORM | Format::R8G8B8A8_SRGB)
}

/// Not a `GpuTexture` (a 2D material bind group): the probe array copies it in.
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
