//! A panorama's light in L2 SH, memoised by path, digest and import settings, a failed decode too.
//! An answer holds, reading no file, until the write log names its file, sidecar or source.

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

/// Enough to switch between a few skies in the editor.
const KEPT: usize = 4;

type Key = (PathBuf, u64, TextureSettings);

struct Remembered {
    freshness: Freshness,
    project: u64,
    answers: Vec<(String, String, String, Option<Sh9>)>,
}

/// Locked `remembered` first, then `projected`.
struct Memo {
    projected: Mutex<Vec<(Key, Option<Sh9>)>>,
    remembered: Mutex<Option<Remembered>>,
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

fn decode(file: &Path, settings: &TextureSettings) -> Option<Sh9> {
    // Uncompressed and taken raw (bytes over 255, no sRGB decode), as the sky pass samples it.
    let loaded = texture_import::load(file, settings, TextureKind::Colour, false)?;
    let Texels::Image(image) = loaded.texels else {
        return None;
    };
    let rgba = image.to_rgba8();
    Some(project(rgba.width(), rgba.height(), rgba.as_raw()))
}

#[cfg(test)]
pub(super) fn decodes() -> usize {
    with_memo(|memo| memo.decodes.load(std::sync::atomic::Ordering::Relaxed))
}

/// Rows kept at most: L2 needs far fewer, and a block sums cheaply where per-texel bases would not.
const ROWS: u32 = 128;

pub(super) fn project(w: u32, h: u32, rgba: &[u8]) -> Sh9 {
    let side = (h / ROWS).max(1);
    let gw = w.div_ceil(side);
    let mut sh = Sh9::zero();
    for (i, (rgb, weight)) in blocks(w, h, rgba, side).into_iter().enumerate() {
        let (bx, by) = (i as u32 % gw, i as u32 / gw);
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

fn blocks(w: u32, h: u32, rgba: &[u8], side: u32) -> Vec<(Vec3, f32)> {
    let (gw, gh) = (w.div_ceil(side), h.div_ceil(side));
    // A texel's solid angle: 2π/w of its row's `cos θ` span; a constant image projects exactly.
    let band = |y: u32| (y as f32 / h as f32 * PI).cos();
    let rows: Vec<f32> = (0..h)
        .map(|y| (band(y) - band(y + 1)) * 2.0 * PI / w as f32)
        .collect();
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
