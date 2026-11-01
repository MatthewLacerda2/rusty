//! A panorama's light (#1675): its equirect texels projected into L2 SH, memoised by
//! the file's path, the digest of its bytes and the import settings the decode reads
//! (the sidecar's Max Size and sRGB), so a changed file or sidecar re-projects. A
//! file that fails to decode is remembered as such under the same key, so a broken
//! panorama is decoded once, not on every frame.
//!
//! The texels are read as the sky pass draws them: decoded and fitted as the
//! renderer's uncompressed upload, then taken raw (`load_panorama` samples the data
//! view, no sRGB decode), each byte over 255. Each texel weighs its solid angle, its
//! direction `common::equirect_uv`'s inverse at its centre.

use std::f32::consts::PI;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use glam::Vec3;

use crate::asset::convert::{source_digest, Seen};
use crate::asset::sidecar::TextureSettings;
use crate::asset::texture_import::{self, Texels, TextureKind};
use crate::asset::MapRef;
use crate::scene::Sh9;

/// How many projected panoramas are kept: enough to switch between a few skies in
/// the editor without re-reading them.
const KEPT: usize = 4;

/// What a projection is keyed by: the file, its bytes' digest, its import settings.
type Key = (PathBuf, u64, TextureSettings);

/// The projected panoramas, newest last: the key, and its SH (`None`: it failed to
/// decode).
static PROJECTED: Mutex<Vec<(Key, Option<Sh9>)>> = Mutex::new(Vec::new());

/// The radiance SH of the panorama `path` names, `None` when it cannot be read.
pub(super) fn radiance(path: &str) -> Option<Sh9> {
    let file = MapRef::from(path).file()?;
    let Seen::Read(digest) = source_digest(&file) else {
        return None;
    };
    let settings = crate::asset::sidecar::texture(&file);
    let key = (file, digest, settings);
    let mut kept = PROJECTED.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((_, sh)) = kept.iter().find(|(k, _)| *k == key) {
        return *sh;
    }
    let sh = decode(&key.0, &settings);
    kept.retain(|((f, _, _), _)| *f != key.0);
    if kept.len() == KEPT {
        kept.remove(0);
    }
    kept.push((key, sh));
    sh
}

/// Decode `file` as the sky pass uploads it and project it; `None` when it fails.
fn decode(file: &Path, settings: &TextureSettings) -> Option<Sh9> {
    #[cfg(test)]
    DECODES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let loaded = texture_import::load(file, settings, TextureKind::Colour, false)?;
    let Texels::Image(image) = loaded.texels else {
        return None;
    };
    let rgba = image.to_rgba8();
    Some(project(rgba.width(), rgba.height(), rgba.as_raw()))
}

/// How many decodes this process ran: a test's handle on the memo.
#[cfg(test)]
static DECODES: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

#[cfg(test)]
pub(super) fn decodes() -> usize {
    DECODES.load(std::sync::atomic::Ordering::Relaxed)
}

/// Rows the projection keeps at most: an L2 projection needs far fewer, and a
/// block of texels sums cheaply where a basis evaluation per texel would not
/// (a 4K panorama is eight million texels, projected in an unoptimised build).
const ROWS: u32 = 128;

/// The radiance SH of a `w` × `h` equirect image of RGBA8 texels, row-major, every
/// texel weighed by its solid angle. Texels are summed in square blocks of up to
/// `h / ROWS` a side, each block projected once from its centre.
pub(super) fn project(w: u32, h: u32, rgba: &[u8]) -> Sh9 {
    let side = (h / ROWS).max(1);
    let (gw, gh) = (w.div_ceil(side), h.div_ceil(side));
    // A row's texels share its band of the sphere exactly: 2π/w of the
    // `cos θ` it spans, so a constant image projects to that constant.
    let row = |y: u32| {
        let band = |y: u32| (y as f32 / h as f32 * PI).cos();
        (band(y) - band(y + 1)) * 2.0 * PI / w as f32
    };
    // Per block: the solid-angle-weighted colour, and the solid angle.
    let mut blocks = vec![(Vec3::ZERO, 0.0f32); (gw * gh) as usize];
    for (i, p) in rgba.chunks_exact(4).enumerate() {
        let (x, y) = (i as u32 % w, i as u32 / w);
        let rgb = Vec3::new(p[0] as f32, p[1] as f32, p[2] as f32) / 255.0;
        let block = &mut blocks[((y / side) * gw + x / side) as usize];
        block.0 += rgb * row(y);
        block.1 += row(y);
    }
    let mut sh = Sh9::zero();
    for (i, (rgb, weight)) in blocks.into_iter().enumerate() {
        let (bx, by) = (i as u32 % gw, i as u32 / gw);
        // The block's centre, as `equirect_uv` maps it: u = (φ + π) / 2π with
        // φ = atan2(z, x), v = θ / π with θ = acos(y).
        let centre = |b: u32, n: u32| ((b * side) as f32 + ((b + 1) * side).min(n) as f32) * 0.5;
        let theta = centre(by, h) / h as f32 * PI;
        let phi = centre(bx, w) / w as f32 * 2.0 * PI - PI;
        let (st, ct) = theta.sin_cos();
        let dir = Vec3::new(st * phi.cos(), ct, st * phi.sin());
        if weight > 0.0 {
            sh.add_sample(dir, rgb / weight, weight);
        }
    }
    sh
}

#[cfg(test)]
#[path = "panorama_tests.rs"]
mod tests;
