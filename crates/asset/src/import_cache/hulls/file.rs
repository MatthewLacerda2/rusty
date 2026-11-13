//! crates/asset/src/import_cache/hulls/file.rs — the bytes of a hull sidecar (#1940).
//!
//! Little-endian, every float stored as its bits, so a read gives back exactly
//! the hulls a decomposition made:
//!
//! ```text
//! "RHUL"  u32 n  n × u8 decomposer  u32 entries
//! entry:  u64 hash  u64 positions  u64 indices  u32 hulls
//! hull:   u32 points  points × (f32 x, y, z)  u32 triangles  triangles × (u32 a, b, c)
//! ```
//!
//! `decomposer` is `core::hulls::DECOMPOSER` as UTF-8: a file another decomposer
//! made (a parry release, V-HACD parameters, the piece filter) is refused, so a
//! machine with the sidecar never simulates other hulls than one without it.
//! An entry with no hulls records a mesh V-HACD found no piece with volume in.
//! A file that is cut short, has a count past its length, a point that is not
//! finite or an index past its hull's points is refused whole: the meshes are then
//! decomposed again, and an authoring build writes the file anew.

use std::collections::BTreeMap;
use std::sync::Arc;

use rusty_core::hulls::{Hull, Key, Pieces, DECOMPOSER};

const MAGIC: &[u8; 4] = b"RHUL";

/// The sidecar bytes of `entries`.
pub fn encode(entries: &BTreeMap<Key, Pieces>) -> Vec<u8> {
    let mut out = MAGIC.to_vec();
    let u32_of = |n: usize| u32::try_from(n).unwrap_or(u32::MAX).to_le_bytes();
    out.extend(u32_of(DECOMPOSER.len()));
    out.extend(DECOMPOSER.as_bytes());
    out.extend(u32_of(entries.len()));
    for (&(hash, positions, indices), pieces) in entries {
        out.extend(hash.to_le_bytes());
        out.extend((positions as u64).to_le_bytes());
        out.extend((indices as u64).to_le_bytes());
        out.extend(u32_of(pieces.len()));
        for hull in pieces.iter() {
            out.extend(u32_of(hull.points.len()));
            let coords = hull.points.iter().flatten();
            out.extend(coords.flat_map(|c| c.to_bits().to_le_bytes()));
            out.extend(u32_of(hull.triangles.len()));
            out.extend(
                hull.triangles
                    .iter()
                    .flatten()
                    .flat_map(|i| i.to_le_bytes()),
            );
        }
    }
    out
}

/// The entries of the sidecar `bytes`, or why the file is refused.
pub fn parse(bytes: &[u8]) -> Result<BTreeMap<Key, Pieces>, String> {
    let mut r = Reader { bytes, at: 0 };
    if r.take(4)? != MAGIC {
        return Err("not a hull sidecar".to_string());
    }
    let made_by = r.count(1)?;
    if r.take(made_by)? != DECOMPOSER.as_bytes() {
        return Err(format!("made by another decomposer than {DECOMPOSER:?}"));
    }
    let mut entries = BTreeMap::new();
    for _ in 0..r.u32()? {
        let hash = r.u64()?;
        let positions = usize::try_from(r.u64()?).map_err(|e| e.to_string())?;
        let indices = usize::try_from(r.u64()?).map_err(|e| e.to_string())?;
        let count = r.count(4)?;
        let pieces = (0..count).map(|_| r.hull()).collect::<Result<_, _>>()?;
        entries.insert((hash, positions, indices), Arc::new(pieces));
    }
    if r.at != bytes.len() {
        return Err(format!("{} bytes past the last entry", bytes.len() - r.at));
    }
    Ok(entries)
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8], String> {
        let end = self.at.checked_add(n).filter(|&e| e <= self.bytes.len());
        let end = end.ok_or_else(|| format!("cut short at byte {}", self.at))?;
        let slice = &self.bytes[self.at..end];
        self.at = end;
        Ok(slice)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], String> {
        let slice = self.take(N)?;
        Ok(std::array::from_fn(|i| slice[i]))
    }

    fn u32(&mut self) -> Result<u32, String> {
        self.array().map(u32::from_le_bytes)
    }

    fn f32(&mut self) -> Result<f32, String> {
        self.u32().map(f32::from_bits)
    }

    fn u64(&mut self) -> Result<u64, String> {
        self.array().map(u64::from_le_bytes)
    }

    /// A count of items at least `size` bytes each, refused when the rest of the
    /// file cannot hold them (so a bad count never allocates).
    fn count(&mut self, size: usize) -> Result<usize, String> {
        let n = self.u32()? as usize;
        if n.saturating_mul(size) > self.bytes.len() - self.at {
            return Err(format!("a count of {n} past the end at byte {}", self.at));
        }
        Ok(n)
    }

    fn hull(&mut self) -> Result<Hull, String> {
        let n = self.count(12)?;
        let mut points = Vec::with_capacity(n);
        for _ in 0..n {
            let p = [self.f32()?, self.f32()?, self.f32()?];
            if !p.iter().all(|c| c.is_finite()) {
                return Err(format!("a point that is not finite: {p:?}"));
            }
            points.push(p);
        }
        let t = self.count(12)?;
        let mut triangles = Vec::with_capacity(t);
        for _ in 0..t {
            let tri = [self.u32()?, self.u32()?, self.u32()?];
            if let Some(&i) = tri.iter().find(|&&i| i as usize >= n) {
                return Err(format!("index {i} past a hull's {n} points"));
            }
            triangles.push(tri);
        }
        Ok(Hull { points, triangles })
    }
}
