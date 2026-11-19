//! A panorama's light (#1675): its equirect texels projected into L2 SH, memoised by
//! the file's path, the digest of its bytes and the import settings the decode reads
//! (the sidecar's Max Size and sRGB), so a changed file or sidecar re-projects. A
//! file that fails to decode is remembered as such under the same key, so a broken
//! panorama is decoded once, not on every frame.
//!
//! A frame reaches that answer with no file access: the last answer per
//! `skybox_path` spelling is kept until the write log (`core::fs::written`) names
//! the panorama, its `.meta` sidecar or, for an extracted texture, its source, or
//! another project opens; only then does a call digest the file and read its
//! sidecar again. A change made outside the engine reaches the log through the
//! editor's file watch, as for textures; a shipped game watches nothing. A
//! panorama that is missing is a miss, re-checked on the write log's retry beat.
//!
//! The texels are read as the sky pass draws them: decoded and fitted as the
//! renderer's uncompressed upload, then taken raw (`load_panorama` samples the data
//! view, no sRGB decode), each byte over 255. Each texel weighs its solid angle, its
//! direction `common::equirect_uv`'s inverse at its centre.

use std::f32::consts::PI;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use glam::Vec3;

use crate::Sh9;
use rusty_asset::convert::{source_digest, texture_ref, Seen};
use rusty_asset::sidecar::{meta_path, TextureSettings};
use rusty_asset::texture_import::{self, Texels, TextureKind};
use rusty_asset::MapRef;
use rusty_core::fs::freshness::Freshness;
use rusty_core::fs::written::Stale;
use rusty_core::project;

/// How many projected panoramas are kept: enough to switch between a few skies in
/// the editor without re-reading them.
const KEPT: usize = 4;

/// What a projection is keyed by: the file, its bytes' digest, its import settings.
type Key = (PathBuf, u64, TextureSettings);

/// The last answer per `skybox_path`, valid until the write log names its file,
/// its sidecar or its source, or another project opens.
struct Remembered {
    freshness: Freshness,
    project: u64,
    /// The spelling asked for, the file it resolved to, that file's sidecar, the SH.
    answers: Vec<(String, String, String, Option<Sh9>)>,
}

/// The panorama memo: what was projected, and what each spelling last answered.
/// Locked in that field order's reverse: `remembered`, then `projected`.
struct Memo {
    /// The projected panoramas, newest last: the key, and its SH (`None`: it
    /// failed to decode).
    projected: Mutex<Vec<(Key, Option<Sh9>)>>,
    remembered: Mutex<Option<Remembered>>,
    /// How many decodes this memo ran: a test's handle on it.
    #[cfg(test)]
    decodes: std::sync::atomic::AtomicUsize,
}

impl Memo {
    const fn new() -> Self {
        Self {
            projected: Mutex::new(Vec::new()),
            remembered: Mutex::new(None),
            #[cfg(test)]
            decodes: std::sync::atomic::AtomicUsize::new(0),
        }
    }
}

/// The process's memo. A test gets one per thread instead, so tests sharing a
/// process (a plain `cargo test`) never evict each other's answers, move each
/// other's retry beat or decode count; the renderer calls from one thread anyway.
#[cfg(not(test))]
static MEMO: Memo = Memo::new();
#[cfg(test)]
thread_local! {
    static MEMO: Memo = const { Memo::new() };
}

fn with_memo<R>(f: impl FnOnce(&Memo) -> R) -> R {
    #[cfg(not(test))]
    return f(&MEMO);
    #[cfg(test)]
    MEMO.with(f)
}

impl Remembered {
    fn new() -> Self {
        Self {
            freshness: Freshness::new(),
            project: project::moves(),
            answers: Vec::new(),
        }
    }

    /// Drop what another project or a noted write made stale: no file access
    /// unless something was written or a miss is due its retry.
    fn refresh(&mut self) {
        if self.project != project::moves() {
            *self = Self::new();
            return;
        }
        let changes = self.freshness.poll_with(texture_ref::exists);
        if changes.stale.is_empty() {
            return;
        }
        let stale = Stale::new(changes.stale);
        self.answers.retain(|(path, file, meta, _)| {
            !(texture_ref::is_written(&stale, path) || stale.matches(file) || stale.matches(meta))
        });
        self.freshness
            .forget_misses_where(|p| texture_ref::is_written(&stale, p));
    }
}

/// The radiance SH of the panorama `path` names, `None` when it cannot be read.
pub(super) fn radiance(path: &str) -> Option<Sh9> {
    with_memo(|memo| radiance_in(memo, path))
}

fn radiance_in(memo: &Memo, path: &str) -> Option<Sh9> {
    let mut remembered = memo.remembered.lock().unwrap_or_else(|e| e.into_inner());
    let remembered = remembered.get_or_insert_with(Remembered::new);
    remembered.refresh();
    if let Some((.., sh)) = remembered.answers.iter().find(|(p, ..)| p == path) {
        return *sh;
    }
    if remembered.freshness.is_miss(path) {
        return None;
    }
    let Some((file, sh)) = projected(memo, path) else {
        remembered.freshness.miss(path);
        return None;
    };
    if remembered.answers.len() == KEPT {
        remembered.answers.remove(0);
    }
    let meta = meta_path(&file).to_string_lossy().into_owned();
    let file = file.to_string_lossy().into_owned();
    remembered.answers.push((path.to_string(), file, meta, sh));
    sh
}

/// The file `path` names and its projected SH (`None`: it failed to decode), keyed
/// by the file's digest and import settings; `None` when there is no file to read.
fn projected(memo: &Memo, path: &str) -> Option<(PathBuf, Option<Sh9>)> {
    let file = MapRef::from(path).file()?;
    let Seen::Read(digest) = source_digest(&file) else {
        return None;
    };
    let settings = rusty_asset::sidecar::texture(&file);
    let key = (file, digest, settings);
    let mut kept = memo.projected.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((_, sh)) = kept.iter().find(|(k, _)| *k == key) {
        return Some((key.0.clone(), *sh));
    }
    #[cfg(test)]
    memo.decodes
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let sh = decode(&key.0, &settings);
    kept.retain(|((f, _, _), _)| *f != key.0);
    if kept.len() == KEPT {
        kept.remove(0);
    }
    let file = key.0.clone();
    kept.push((key, sh));
    Some((file, sh))
}

/// Decode `file` as the sky pass uploads it and project it; `None` when it fails.
fn decode(file: &Path, settings: &TextureSettings) -> Option<Sh9> {
    let loaded = texture_import::load(file, settings, TextureKind::Colour, false)?;
    let Texels::Image(image) = loaded.texels else {
        return None;
    };
    let rgba = image.to_rgba8();
    Some(project(rgba.width(), rgba.height(), rgba.as_raw()))
}

/// How many decodes this thread's memo ran.
#[cfg(test)]
pub(super) fn decodes() -> usize {
    with_memo(|memo| memo.decodes.load(std::sync::atomic::Ordering::Relaxed))
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
    let gw = w.div_ceil(side);
    let mut sh = Sh9::zero();
    for (i, (rgb, weight)) in blocks(w, h, rgba, side).into_iter().enumerate() {
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

/// Per block of `side` × `side` texels, row-major: the solid-angle-weighted
/// colour and the solid angle it covers.
fn blocks(w: u32, h: u32, rgba: &[u8], side: u32) -> Vec<(Vec3, f32)> {
    let (gw, gh) = (w.div_ceil(side), h.div_ceil(side));
    // A row's texels share its band of the sphere exactly: 2π/w of the
    // `cos θ` it spans, so a constant image projects to that constant. One
    // weight per row, computed once: it depends on nothing else.
    let band = |y: u32| (y as f32 / h as f32 * PI).cos();
    let rows: Vec<f32> = (0..h)
        .map(|y| (band(y) - band(y + 1)) * 2.0 * PI / w as f32)
        .collect();
    // Per block: the solid-angle-weighted colour, and the solid angle.
    let mut blocks = vec![(Vec3::ZERO, 0.0f32); (gw * gh) as usize];
    for (i, p) in rgba.chunks_exact(4).enumerate() {
        let (x, y) = (i as u32 % w, i as u32 / w);
        let rgb = Vec3::new(p[0] as f32, p[1] as f32, p[2] as f32) / 255.0;
        let block = &mut blocks[((y / side) * gw + x / side) as usize];
        let weight = rows[y as usize];
        block.0 += rgb * weight;
        block.1 += weight;
    }
    blocks
}

#[cfg(test)]
#[path = "panorama_tests.rs"]
mod tests;
