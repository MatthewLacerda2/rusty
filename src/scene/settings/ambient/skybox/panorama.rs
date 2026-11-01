//! A panorama's light (#1675): its equirect texels projected into L2 SH, memoised by
//! the file's path and the digest of its bytes, so a changed file re-projects.
//!
//! The texels are read as the sky pass draws them: decoded and fitted as the
//! renderer's uncompressed upload, then taken raw (`load_panorama` samples the data
//! view, no sRGB decode), each byte over 255. Each texel weighs its solid angle, its
//! direction `common::equirect_uv`'s inverse at its centre.

use std::f32::consts::PI;
use std::path::PathBuf;
use std::sync::Mutex;

use glam::Vec3;

use crate::asset::convert::{source_digest, Seen};
use crate::asset::texture_import::{self, Texels, TextureKind};
use crate::asset::MapRef;
use crate::scene::Sh9;

/// How many projected panoramas are kept: enough to switch between a few skies in
/// the editor without re-reading them.
const KEPT: usize = 4;

/// The projected panoramas, newest last: the file, its bytes' digest, its SH.
static PROJECTED: Mutex<Vec<(PathBuf, u64, Sh9)>> = Mutex::new(Vec::new());

/// The radiance SH of the panorama `path` names, `None` when it cannot be read.
pub(super) fn radiance(path: &str) -> Option<Sh9> {
    let file = MapRef::from(path).file()?;
    let Seen::Read(digest) = source_digest(&file) else {
        return None;
    };
    let mut kept = PROJECTED.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((_, _, sh)) = kept.iter().find(|(f, d, _)| *f == file && *d == digest) {
        return Some(*sh);
    }
    let settings = crate::asset::sidecar::texture(&file);
    let loaded = texture_import::load(&file, &settings, TextureKind::Colour, false)?;
    let Texels::Image(image) = loaded.texels else {
        return None;
    };
    let sh = project(&image.to_rgba8());
    kept.retain(|(f, _, _)| *f != file);
    if kept.len() == KEPT {
        kept.remove(0);
    }
    kept.push((file, digest, sh));
    Some(sh)
}

/// The radiance SH of an equirect image, every texel weighed by its solid angle.
pub(super) fn project(image: &image::RgbaImage) -> Sh9 {
    let (w, h) = image.dimensions();
    let mut sh = Sh9::zero();
    // A row's texels share its band of the sphere exactly: 2π/w of the
    // `cos θ` it spans, so a constant image projects to that constant.
    let row = |y: u32| {
        let band = |y: u32| (y as f32 / h as f32 * PI).cos();
        (band(y) - band(y + 1)) * 2.0 * PI / w as f32
    };
    for (x, y, p) in image.enumerate_pixels() {
        // `equirect_uv`: u = (φ + π) / 2π with φ = atan2(z, x), v = θ / π with
        // θ = acos(y).
        let theta = (y as f32 + 0.5) / h as f32 * PI;
        let phi = (x as f32 + 0.5) / w as f32 * 2.0 * PI - PI;
        let (st, ct) = theta.sin_cos();
        let dir = Vec3::new(st * phi.cos(), ct, st * phi.sin());
        let rgb = Vec3::new(p[0] as f32, p[1] as f32, p[2] as f32) / 255.0;
        sh.add_sample(dir, rgb, row(y));
    }
    sh
}
