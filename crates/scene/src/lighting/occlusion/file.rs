//! The `<scene>.occlusion` file (#1714): the baked PVS beside the scene, as the
//! lightmap pages and the SH sidecar sit beside it. Binary and little-endian, read
//! whole: `RPVS`, the objects (entity `u32`, fingerprint `u64`, occluder `u8`), the
//! grid (origin `3 × f32`, cell `f32`, dims `3 × u32`), the rows' words (a `u32`
//! count, then `u64`s) and the cells' rows (a `u32` count, then `u32`s).
//!
//! A file that is truncated, holds a value out of range or a NaN, or disagrees with
//! itself is refused whole: the scene then culls nothing by occlusion, and says so.

use std::path::PathBuf;
use std::sync::Arc;

use glam::{UVec3, Vec3};

use super::{BakedObject, Grid, OcclusionData, MAX_CELLS};
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

/// The data `bytes` hold, or why they are refused.
pub fn decode(bytes: &[u8]) -> Result<OcclusionData, String> {
    let mut r = Reader(bytes);
    if r.take(4)? != MAGIC {
        return Err("not an occlusion file".into());
    }
    let count = r.u32()? as usize;
    let mut objects = Vec::with_capacity(count.min(bytes.len() / 13));
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
    let origin = Vec3::new(r.f32()?, r.f32()?, r.f32()?);
    let cell = r.f32()?;
    let dims = UVec3::new(r.u32()?, r.u32()?, r.u32()?);
    // Checked: a bad file is refused whole, never a panic in a build with overflow
    // checks.
    let cells_in = u64::from(dims.x)
        .checked_mul(u64::from(dims.y))
        .and_then(|n| n.checked_mul(u64::from(dims.z)));
    let finite = origin.is_finite() && cell.is_finite() && cell > 0.0;
    if !finite || cells_in.is_none_or(|n| n > MAX_CELLS as u64) {
        return Err("grid out of range".into());
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
    OcclusionData::new(grid, objects, rows, cells)
        .ok_or_else(|| "rows disagree with the grid".into())
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
