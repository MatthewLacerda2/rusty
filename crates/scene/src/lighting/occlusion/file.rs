//! The `<scene>.occlusion` file (#1714): the baked PVS beside the scene, as the
//! lightmap pages and the SH sidecar sit beside it. Binary and little-endian, read
//! whole: `RPVS`, the bake's stamp (its version `u32`, its never-culled count
//! `u32`, its floor source `u32`, its baked cells `u32`, its reach hash `u64`, the
//! bounds its grid was laid over `6 × f32`), the objects (entity `u32`, fingerprint `u64`, occluder `u8`), the
//! grid (origin `3 × f32`, cell `f32`, dims `3 × u32`), the rows' words (a `u32`
//! count, then `u64`s) and the cells' rows (a `u32` count, then `u32`s).
//!
//! A file that is truncated, holds a value out of range or a NaN, or disagrees with
//! itself is refused whole: the scene then culls nothing by occlusion, and says so.

use std::path::PathBuf;
use std::sync::Arc;

use glam::{UVec3, Vec3};

use super::{BakeStamp, BakedObject, FloorSource, Grid, OcclusionData, MAX_GRID_CELLS};
use crate::io::Saved;
use crate::Scene;

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

/// The data `bytes` hold, or why they are refused.
pub fn decode(bytes: &[u8]) -> Result<OcclusionData, String> {
    let mut r = Reader(bytes);
    if r.take(4)? != MAGIC {
        return Err("not an occlusion file".into());
    }
    let (version, never_culled) = (r.u32()?, r.u32()?);
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
    if cells_in.is_some_and(|n| u64::from(stamp.baked) > n) {
        return Err("more cells baked than the grid holds".into());
    }
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

/// Bring the PVS file beside `scene_path` to what `scene` holds, by the bake
/// files' rule (`scene::io::BakeFiles`, #1982): written only when this world baked
/// since it last read or wrote the file, or holds other bytes than it did, and never
/// over a newer file with bytes it only loaded. A file it could not read stays
/// until it bakes; one it refused is rewritten or removed ([`load_sidecar`]).
pub fn save_sidecar(scene: &Scene, scene_path: &str) -> Result<Saved, String> {
    let path = occlusion_path(scene_path);
    let want = scene.occlusion.as_deref().map(encode);
    scene.bake_files.store(&path, want.as_deref(), "Occlusion")
}

/// Load the PVS beside `scene_path` onto `scene`: none when there is no file, and
/// none, with a warning, when the file is refused (the next save then rewrites or
/// removes it, while it is still that file). A file that is there but cannot be read (a permission, a transient
/// failure) also loads none, with a warning, and the next save keeps it.
pub fn load_sidecar(scene: &mut Scene, scene_path: &str) {
    apply_sidecar(scene, read_sidecar(scene_path));
}

/// What a load found at `<scene>.occlusion`, read and decoded from the file alone
/// ([`read_sidecar`]) and put on a scene later ([`apply_sidecar`]), so a load can
/// do the work before it touches the live scene (#2123).
pub struct OcclusionFile {
    path: PathBuf,
    read: Read,
}

enum Read {
    Missing,
    Unreadable(std::io::Error),
    Refused(Vec<u8>, String),
    /// The file's bytes, what a save would write (encoded afresh: a file that does
    /// not round-trip byte for byte never reads as a bake since), and the PVS.
    Decoded(Vec<u8>, Vec<u8>, OcclusionData),
}

/// Read and decode the PVS beside `scene_path`.
pub fn read_sidecar(scene_path: &str) -> OcclusionFile {
    let path = occlusion_path(scene_path);
    let read = match std::fs::read(&path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Read::Missing,
        Err(e) => Read::Unreadable(e),
        Ok(bytes) => match decode(&bytes) {
            Err(e) => Read::Refused(bytes, e),
            Ok(data) => Read::Decoded(bytes, encode(&data), data),
        },
    };
    OcclusionFile { path, read }
}

/// Put what [`read_sidecar`] found on `scene` ([`load_sidecar`] says what each
/// outcome loads).
pub fn apply_sidecar(scene: &mut Scene, file: OcclusionFile) {
    let OcclusionFile { path, read } = file;
    *scene.occlusion = None;
    match read {
        Read::Missing => scene.bake_files.loaded(&path, None, None),
        Read::Unreadable(e) => {
            log::warn!(
                "[Occlusion] could not read {} ({e}): nothing is occlusion-culled, and \
                 the file is kept",
                path.display()
            );
            scene.bake_files.unreadable(&path)
        }
        Read::Refused(bytes, e) => {
            log::warn!(
                "[Occlusion] {} refused ({e}): nothing is occlusion-culled, and the next \
                 save removes it",
                path.display()
            );
            scene.bake_files.refused(&path, &bytes)
        }
        Read::Decoded(bytes, held, data) => {
            scene.bake_files.loaded(&path, Some(&bytes), Some(&held));
            *scene.occlusion = Some(Arc::new(data));
        }
    }
}

impl Scene {
    /// Put a bake's PVS on the scene (`None`: the bake kept none): the one place a
    /// bake's output lands, so its next save writes it, even over a newer file
    /// another process baked (#1982).
    pub fn put_baked_occlusion(&mut self, data: Option<OcclusionData>) {
        self.bake_files.baked(SUFFIX);
        *self.occlusion = data.map(Arc::new);
    }

    /// A bake found the PVS the scene holds already current and kept it (#2348):
    /// it owns its file as a bake's output does, so the next save writes it there,
    /// even over a newer file another process wrote (the bytes a re-bake would
    /// write).
    pub fn keep_baked_occlusion(&mut self) {
        self.bake_files.baked(SUFFIX);
    }
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
