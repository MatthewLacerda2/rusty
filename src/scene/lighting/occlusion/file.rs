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
use crate::scene::Scene;

const MAGIC: &[u8; 4] = b"RPVS";
const SUFFIX: &str = ".occlusion";

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
    let cells_in = dims.x as u64 * dims.y as u64 * dims.z as u64;
    let finite = origin.is_finite() && cell.is_finite() && cell > 0.0;
    if !finite || cells_in > MAX_CELLS as u64 {
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

/// Write `scene`'s PVS beside `scene_path`, or remove a stale one when it has none.
pub fn save_sidecar(scene: &Scene, scene_path: &str) -> Result<(), String> {
    let path = occlusion_path(scene_path);
    let Some(data) = scene.occlusion.as_deref() else {
        if path.exists() {
            std::fs::remove_file(&path)
                .map_err(|e| format!("Failed to remove {}: {e}", path.display()))?;
        }
        return Ok(());
    };
    // lint: allow(asset-write) the baked PVS, a lighting document read whole on load
    std::fs::write(&path, encode(data))
        .map_err(|e| format!("Failed to write {}: {e}", path.display()))
}

/// Load the PVS beside `scene_path` onto `scene`: none when there is no file, and
/// none, with a warning, when the file is refused.
pub fn load_sidecar(scene: &mut Scene, scene_path: &str) {
    let path = occlusion_path(scene_path);
    let data = match std::fs::read(&path) {
        Ok(bytes) => decode(&bytes)
            .map_err(|e| {
                log::warn!(
                    "[Occlusion] {} refused ({e}): nothing is occlusion-culled",
                    path.display()
                )
            })
            .ok(),
        Err(_) => None,
    };
    *scene.occlusion = data.map(Arc::new);
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
