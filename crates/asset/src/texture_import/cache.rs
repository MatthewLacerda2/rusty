//! A compressed texture in the import cache (`.bctex`); a stale, unparseable or wrongly sized file is a miss, never an upload of the wrong length.
//!
//! Layout, little-endian: `RBCT`, stamp (u64), format (u8, index in [`BlockFormat::ALL`]), width, height, level count (u32 each), [`VarianceTable`] (16 bytes), then each level's byte length (u32) and blocks.

use std::path::Path;

use super::encode::{full_chain_len, BlockFormat, Compressed};
use super::refresh::Uncached;
use super::variance::{VarianceTable, VARIANCE_LEVELS};
use super::{TextureSettings, ENCODER_VERSION};
use crate::convert::{source_digest, Seen};
use rusty_core::hash::content;

const MAGIC: &[u8; 4] = b"RBCT";

/// Digest of the source bytes, the settings that shape the blocks (not wrap or filter) and the encoder's version.
pub(super) fn stamp(source: u64, settings: &TextureSettings) -> u64 {
    let made_by = format!(
        "{source:016x} v{ENCODER_VERSION} {} {} {} {} {}",
        settings.texture_type.name(),
        super::is_linear(settings),
        settings.compression.name(),
        settings.max_size,
        settings.generate_mipmaps,
    );
    content(made_by.as_bytes())
}

type Head = (BlockFormat, u32, u32, u32, VarianceTable);

fn head(r: &mut Reader<'_>, stamp: u64) -> Option<Head> {
    if r.take(4)? != MAGIC || r.u64()? != stamp {
        return None;
    }
    let format = *BlockFormat::ALL.get(usize::from(r.take(1)?[0]))?;
    let (width, height, count) = (r.u32()?, r.u32()?, r.u32()?);
    let whole = width > 0 && height > 0 && width % 4 == 0 && height % 4 == 0;
    let fits = count > 0 && count <= full_chain_len(width, height);
    let variance = r.take(VARIANCE_LEVELS)?.try_into().ok()?;
    (whole && fits).then_some((format, width, height, count, variance))
}

const HEAD_LEN: usize = 4 + 8 + 1 + 4 * 3 + VARIANCE_LEVELS;

pub(super) fn is_current(path: &Path, stamp: u64) -> bool {
    use std::io::{Read, Seek, SeekFrom};
    let current = || -> Option<()> {
        let mut file = std::fs::File::open(path).ok()?;
        let len = file.metadata().ok()?.len();
        let mut bytes = [0; HEAD_LEN];
        file.read_exact(&mut bytes).ok()?;
        let head = head(&mut Reader(&bytes), stamp)?;
        walk(&mut file, len, head, |file, want| {
            file.seek(SeekFrom::Current(want as i64)).ok().map(drop)
        })
    };
    current().is_some()
}

/// Each level goes into its own buffer, so the file is never held twice; a malformed head allocates nothing.
pub(super) fn read(path: &Path, stamp: u64) -> Option<Compressed> {
    use std::io::Read;
    let file = std::fs::File::open(path).ok()?;
    let len = file.metadata().ok()?.len();
    let mut file = std::io::BufReader::new(file);
    let mut bytes = [0; HEAD_LEN];
    file.read_exact(&mut bytes).ok()?;
    let head = head(&mut Reader(&bytes), stamp)?;
    let (format, width, height, count, variance) = head;
    let mut levels = Vec::with_capacity(count as usize);
    walk(&mut file, len, head, |file, want| {
        let mut level = vec![0; want];
        file.read_exact(&mut level).ok()?;
        levels.push(level);
        Some(())
    })?;
    Some(Compressed {
        format,
        width,
        height,
        levels,
        variance,
    })
}

fn walk<F: std::io::Read>(
    file: &mut F,
    len: u64,
    (format, width, height, count, _): Head,
    mut level: impl FnMut(&mut F, usize) -> Option<()>,
) -> Option<()> {
    let mut left = len.checked_sub(HEAD_LEN as u64)?;
    for mip in 0..count {
        let mut bytes = [0; 4];
        file.read_exact(&mut bytes).ok()?;
        let side = |s: u32| (s >> mip).max(1);
        let want = format.level_bytes(side(width), side(height));
        left = left.checked_sub(4 + want as u64)?;
        if u32::from_le_bytes(bytes) as usize != want {
            return None;
        }
        level(file, want)?;
    }
    (left == 0).then_some(())
}

pub(super) fn write(path: &Path, stamp: u64, texture: &Compressed) -> std::io::Result<()> {
    let mut out = MAGIC.to_vec();
    out.extend(stamp.to_le_bytes());
    let format = BlockFormat::ALL.iter().position(|f| *f == texture.format);
    out.push(format.unwrap_or_default() as u8);
    let count = texture.levels.len() as u32;
    for n in [texture.width, texture.height, count] {
        out.extend(n.to_le_bytes());
    }
    out.extend(texture.variance);
    for level in &texture.levels {
        out.extend((level.len() as u32).to_le_bytes());
        out.extend(level);
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    // lint: allow(asset-write) a cache only `compressed` reads, by its stamp, never by path watch
    rusty_core::fs::write_atomic(path, &out)
}

/// Test hook: every block zero, nothing encoded; `Err` where an import would make no cache.
#[doc(hidden)]
pub fn write_blank(source: &Path, settings: &TextureSettings) -> Result<(), String> {
    let Seen::Read(digest) = source_digest(source) else {
        return Err(format!("{} cannot be read", source.display()));
    };
    let format = match super::refresh::cached_as(source, settings) {
        Ok(cached) => cached.format,
        Err(Uncached::Left(why)) => return Err(format!("an import makes no cache: {why}")),
        Err(Uncached::Unreadable(e)) => return Err(e),
    };
    let (_, (width, height)) = super::imported_size(source, settings).ok_or("no size")?;
    let count = match settings.generate_mipmaps {
        true => full_chain_len(width, height),
        false => 1,
    };
    let side = |s: u32, mip: u32| (s >> mip).max(1);
    let levels = (0..count)
        .map(|mip| vec![0; format.level_bytes(side(width, mip), side(height, mip))])
        .collect();
    let texture = Compressed {
        format,
        width,
        height,
        levels,
        variance: VarianceTable::default(),
    };
    let path = super::cache_file(source)?;
    write(&path, stamp(digest, settings), &texture).map_err(|e| e.to_string())
}

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
