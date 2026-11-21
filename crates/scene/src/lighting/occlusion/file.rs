//! The `<scene>.occlusion` file (#1714): the baked PVS beside the scene, as the
//! lightmap pages and the SH sidecar sit beside it. Binary and little-endian, read
//! whole: `RPVS`, the bake's stamp (its version `u32`, [`OCCLUSION_VERSION`]: the
//! first field after the magic in every layout, so a decoder reads it before
//! anything else and refuses another version's file as that; its never-culled count
//! `u32`, its floor source `u32`, its baked cells `u32`, its reach hash `u64`, the
//! bounds its grid was laid over `6 × f32`), the objects (entity `u32`, fingerprint `u64`, occluder `u8`), the
//! grid (origin `3 × f32`, cell `f32`, dims `3 × u32`), the rows' words (a `u32`
//! count, then `u64`s) and the cells' rows (a `u32` count, then `u32`s).
//!
//! A file that is truncated, holds a value out of range or a NaN, or disagrees with
//! itself is refused whole: the scene then culls nothing by occlusion, and says so
//! (`sidecar`, which reads and writes the file beside the scene).

use std::path::PathBuf;

use glam::{UVec3, Vec3};

use super::{
    BakeStamp, BakedObject, FloorSource, Grid, OcclusionData, MAX_GRID_CELLS, OCCLUSION_VERSION,
};

const MAGIC: &[u8; 4] = b"RPVS";
/// What `<scene>` takes to name its PVS file.
pub const SUFFIX: &str = ".occlusion";

/// Where `scene_path`'s PVS is kept: `foo.scene` → `foo.scene.occlusion`.
pub fn occlusion_path(scene_path: &str) -> PathBuf {
    PathBuf::from(format!("{scene_path}{SUFFIX}"))
}

/// `data` as the file holds it.
pub fn encode(data: &OcclusionData) -> Vec<u8> {
    let (rows, cells) = data.parts();
    let mut out = Vec::with_capacity(data.byte_size());
    out.extend_from_slice(MAGIC);
    put(&mut out, data.stamp.version);
    put(&mut out, data.stamp.never_culled);
    put(&mut out, data.stamp.floor.code());
    put(&mut out, data.stamp.baked);
    out.extend(data.stamp.reach.to_le_bytes());
    let (lo, hi) = data.stamp.bounds;
    for f in [lo.x, lo.y, lo.z, hi.x, hi.y, hi.z] {
        out.extend(f.to_le_bytes());
    }
    put(&mut out, data.objects.len() as u32);
    for o in &data.objects {
        put(&mut out, o.entity);
        out.extend(o.fingerprint.to_le_bytes());
        out.push(u8::from(o.occluder));
    }
    let g = &data.grid;
    for f in [g.origin.x, g.origin.y, g.origin.z, g.cell] {
        out.extend(f.to_le_bytes());
    }
    [g.dims.x, g.dims.y, g.dims.z]
        .iter()
        .for_each(|&d| put(&mut out, d));
    put(&mut out, rows.len() as u32);
    rows.iter().for_each(|w| out.extend(w.to_le_bytes()));
    put(&mut out, cells.len() as u32);
    cells.iter().for_each(|&c| put(&mut out, c));
    out
}

/// The length of [`encode`]'s bytes for `data`, without encoding it: the header,
/// stamp and counts (92 bytes), then 13 per object, 8 per row word, 4 per cell.
pub fn encoded_len(data: &OcclusionData) -> usize {
    let (rows, cells) = data.parts();
    92 + data.objects.len() * 13 + rows.len() * 8 + cells.len() * 4
}

/// The bake version an occlusion file's bytes were written by, read before
/// anything else of them: `None` when they are no occlusion file.
pub fn version_of(bytes: &[u8]) -> Option<u32> {
    let head = bytes.get(..8).filter(|h| &h[..4] == MAGIC)?;
    Some(u32::from_le_bytes(
        head[4..8].try_into().expect("four bytes"),
    ))
}

/// Why a file of bake `version` is refused by this build, which reads and writes
/// [`OCCLUSION_VERSION`] alone.
fn version_refused(version: u32) -> String {
    let age = if version < OCCLUSION_VERSION {
        "an older"
    } else {
        "a newer"
    };
    rebake(&format!(
        "{age} occlusion bake (version {version}, this build writes {OCCLUSION_VERSION})"
    ))
}

/// Why a file from before the version field is refused: its first `u32` is its
/// object count, so read as a version it would tell an older file as "newer" (or,
/// at a count of [`OCCLUSION_VERSION`], fail on a later field).
fn unversioned() -> String {
    rebake("an occlusion bake from an older engine, which wrote no version")
}

/// `what` was refused, and what clears it.
fn rebake(what: &str) -> String {
    format!(
        "{what}: nothing is occlusion-culled until it is re-baked (Generate Lighting or \
         Bake Occlusion)"
    )
}

/// Whether `bytes` are laid out as the occlusion file was before it held a version:
/// `RPVS`, the objects (a `u32` count, 13 bytes each), the grid (28 bytes), the rows'
/// words (a `u32` count, 8 bytes each) and the cells (a `u32` count, 4 bytes each),
/// ending exactly at the file's end. Its counts say how long it is, so a versioned
/// file matches it only by an exact coincidence of lengths.
fn is_unversioned(bytes: &[u8]) -> bool {
    let count_at = |at: usize| -> Option<usize> {
        let b = bytes.get(at..at.checked_add(4)?)?;
        Some(u32::from_le_bytes(b.try_into().ok()?) as usize)
    };
    let walk = || -> Option<usize> {
        let mut at: usize = 4;
        at = at
            .checked_add(4)?
            .checked_add(count_at(at)?.checked_mul(13)?)?;
        at = at.checked_add(28)?;
        at = at
            .checked_add(4)?
            .checked_add(count_at(at)?.checked_mul(8)?)?;
        at.checked_add(4)?
            .checked_add(count_at(at)?.checked_mul(4)?)
    };
    bytes.starts_with(MAGIC) && walk() == Some(bytes.len())
}

/// Why [`read`] refused an occlusion file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Refusal {
    /// Written by another bake version: an older or a newer one, or from before
    /// the version field ([`unversioned`]). Re-baking is what clears it.
    OtherVersion(String),
    /// Of this version, or no occlusion file, and malformed.
    Malformed(String),
}

/// The data `bytes` hold, or why they are refused. A file another bake version
/// wrote is refused by its version before any other field is read, and one from
/// before the version field ([`is_unversioned`]) as that, whatever its first `u32`.
/// The one place a refusal is told as another version's.
pub(super) fn read(bytes: &[u8]) -> Result<OcclusionData, Refusal> {
    decode_versioned(bytes).map_err(|why| {
        if is_unversioned(bytes) {
            Refusal::OtherVersion(unversioned())
        } else if version_of(bytes).is_some_and(|v| v != OCCLUSION_VERSION) {
            Refusal::OtherVersion(why)
        } else {
            Refusal::Malformed(why)
        }
    })
}

/// The data `bytes` hold, or why they are refused, as text (`read`, which tells
/// another version's file from a malformed one).
pub fn decode(bytes: &[u8]) -> Result<OcclusionData, String> {
    read(bytes).map_err(|refusal| match refusal {
        Refusal::OtherVersion(why) | Refusal::Malformed(why) => why,
    })
}

/// [`decode`] for a file of the versioned layout.
fn decode_versioned(bytes: &[u8]) -> Result<OcclusionData, String> {
    let mut r = Reader(bytes);
    if r.take(4)? != MAGIC {
        return Err("not an occlusion file".into());
    }
    let version = r.u32()?;
    if version != OCCLUSION_VERSION {
        return Err(version_refused(version));
    }
    let never_culled = r.u32()?;
    let floor = FloorSource::of_code(r.u32()?).ok_or("floor source out of range")?;
    let (baked, reach) = (r.u32()?, u64::from_le_bytes(r.array()?));
    let lo = Vec3::new(r.f32()?, r.f32()?, r.f32()?);
    let hi = Vec3::new(r.f32()?, r.f32()?, r.f32()?);
    if !(lo.is_finite() && hi.is_finite()) {
        return Err("bounds not finite".into());
    }
    let stamp = BakeStamp {
        version,
        never_culled,
        floor,
        baked,
        reach,
        bounds: (lo, hi),
    };
    let objects = read_objects(&mut r, bytes.len(), stamp)?;
    let origin = Vec3::new(r.f32()?, r.f32()?, r.f32()?);
    let cell = r.f32()?;
    let dims = UVec3::new(r.u32()?, r.u32()?, r.u32()?);
    // Checked: a bad file is refused whole, never a panic in a build with overflow
    // checks.
    let cells_in = u64::from(dims.x)
        .checked_mul(u64::from(dims.y))
        .and_then(|n| n.checked_mul(u64::from(dims.z)));
    let finite = origin.is_finite() && cell.is_finite() && cell > 0.0;
    if !finite || cells_in.is_none_or(|n| n > MAX_GRID_CELLS as u64) {
        return Err("grid out of range".into());
    }
    stamp_fits(&stamp, cells_in.unwrap_or(0))?;
    let words = r.u32()? as usize;
    let rows = (0..words).map(|_| r.array().map(u64::from_le_bytes));
    let rows = rows.collect::<Result<Vec<_>, _>>()?;
    let cells = r.u32()? as usize;
    let cells = (0..cells).map(|_| r.u32()).collect::<Result<Vec<_>, _>>()?;
    if !r.0.is_empty() {
        return Err("trailing bytes".into());
    }
    let grid = Grid { origin, cell, dims };
    let data = OcclusionData::new(grid, objects, rows, cells);
    let data = data.ok_or("rows disagree with the grid")?;
    Ok(data.stamped(stamp))
}

/// Refuse a stamp that disagrees with its grid of `all` cells or with itself:
/// more cells baked than there are, or one that sampled every cell
/// ([`FloorSource::Everywhere`]) with a reach hash or fewer cells baked.
fn stamp_fits(stamp: &BakeStamp, all: u64) -> Result<(), String> {
    let baked = u64::from(stamp.baked);
    if baked > all {
        return Err("more cells baked than the grid holds".into());
    }
    if stamp.floor == FloorSource::Everywhere && (stamp.reach != 0 || baked != all) {
        return Err("a stamp sampling every cell with a reach or fewer cells baked".into());
    }
    Ok(())
}

/// The baked objects after the stamp, refused when `stamp` counts more never
/// culled than there are; `len` the file's, to bound what a bad count reserves.
fn read_objects(r: &mut Reader, len: usize, stamp: BakeStamp) -> Result<Vec<BakedObject>, String> {
    let count = r.u32()? as usize;
    let mut objects = Vec::with_capacity(count.min(len / 13));
    for _ in 0..count {
        let entity = r.u32()?;
        let fingerprint = u64::from_le_bytes(r.array()?);
        let occluder = match r.take(1)?[0] {
            0 => false,
            1 => true,
            other => return Err(format!("occluder flag {other}")),
        };
        objects.push(BakedObject {
            entity,
            fingerprint,
            occluder,
        });
    }
    if stamp.never_culled as usize > objects.len() {
        return Err(format!(
            "{} never culled of {count} objects",
            stamp.never_culled
        ));
    }
    Ok(objects)
}

fn put(out: &mut Vec<u8>, v: u32) {
    out.extend(v.to_le_bytes());
}

/// The bytes left to read.
struct Reader<'a>(&'a [u8]);

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        if self.0.len() < n {
            return Err("truncated".into());
        }
        let (head, rest) = self.0.split_at(n);
        self.0 = rest;
        Ok(head)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], String> {
        Ok(self.take(N)?.try_into().expect("took N bytes"))
    }

    fn u32(&mut self) -> Result<u32, String> {
        self.array().map(u32::from_le_bytes)
    }

    fn f32(&mut self) -> Result<f32, String> {
        self.array().map(f32::from_le_bytes)
    }
}
