//! A compressed texture in the import cache: a small binary file (`.bctex`)
//! holding the blocks of every level, headed by the [`stamp`] of what made it.
//! A file whose stamp differs, that does not parse, or whose levels do not have
//! the sizes its format and base size give, is a miss, never an upload of the
//! wrong length.
//!
//! Layout, little-endian: `RBCT`, the stamp (u64), the format (u8, its index in
//! [`BlockFormat::ALL`]), width, height and level count (u32 each), then each
//! level's byte length (u32) and its blocks.

use std::path::Path;

use super::encode::{full_chain_len, BlockFormat, Compressed};
use super::{TextureSettings, ENCODER_VERSION};
use crate::core::hash::fnv1a64;

const MAGIC: &[u8; 4] = b"RBCT";

/// A digest of everything the blocks depend on: the source's bytes, the settings
/// that shape them (not wrap or filter, which only the sampler reads) and the
/// encoder's version.
pub(super) fn stamp(source: &[u8], settings: &TextureSettings) -> u64 {
    let made_by = format!(
        "{:016x} v{ENCODER_VERSION} {} {} {} {}",
        fnv1a64(source),
        settings.texture_type.name(),
        settings.compression.name(),
        settings.max_size,
        settings.generate_mipmaps,
    );
    fnv1a64(made_by.as_bytes())
}

/// The texture cached at `path`, when it was made with `stamp`.
pub(super) fn read(path: &Path, stamp: u64) -> Option<Compressed> {
    let bytes = std::fs::read(path).ok()?;
    let mut r = Reader(&bytes);
    if r.take(4)? != MAGIC || r.u64()? != stamp {
        return None;
    }
    let format = *BlockFormat::ALL.get(usize::from(r.take(1)?[0]))?;
    let (width, height, count) = (r.u32()?, r.u32()?, r.u32()?);
    let whole = width > 0 && height > 0 && width % 4 == 0 && height % 4 == 0;
    if !whole || count == 0 || count > full_chain_len(width, height) {
        return None;
    }
    let mut levels = Vec::with_capacity(count as usize);
    for mip in 0..count {
        let len = r.u32()? as usize;
        let side = |s: u32| (s >> mip).max(1);
        if len != format.level_bytes(side(width), side(height)) {
            return None;
        }
        levels.push(r.take(len)?.to_vec());
    }
    r.0.is_empty().then_some(Compressed {
        format,
        width,
        height,
        levels,
    })
}

/// Cache `texture` at `path` under `stamp`, atomically, so a reader in another
/// process never finds half a file.
pub(super) fn write(path: &Path, stamp: u64, texture: &Compressed) -> std::io::Result<()> {
    let mut out = MAGIC.to_vec();
    out.extend(stamp.to_le_bytes());
    let format = BlockFormat::ALL.iter().position(|f| *f == texture.format);
    out.push(format.unwrap_or_default() as u8);
    let count = texture.levels.len() as u32;
    for n in [texture.width, texture.height, count] {
        out.extend(n.to_le_bytes());
    }
    for level in &texture.levels {
        out.extend((level.len() as u32).to_le_bytes());
        out.extend(level);
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    // lint: allow(asset-write) a cache only `compressed` reads, by its stamp, never by path watch
    crate::core::fs::write_atomic(path, &out)
}

/// Reads the cache file front to back.
struct Reader<'a>(&'a [u8]);

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let (head, rest) = self.0.split_at_checked(n)?;
        self.0 = rest;
        Some(head)
    }

    fn u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }

    fn u64(&mut self) -> Option<u64> {
        Some(u64::from_le_bytes(self.take(8)?.try_into().ok()?))
    }
}
