//! A progressive bake's preview on the live scene (#1970, Unity's Progressive
//! Lightmapper). After each pass the bake's worker hands the lightmaps as they
//! stand to an encoder thread of its own ([`Encoder`]), so the trace goes on at
//! once; the encoder writes them beside the scene as `lightmap_preview_*` pages,
//! cheaply (uncompressed PNGs of one level, which the renderer loads as it loads
//! any `.png` page), and the editor's next frame points the scene at them
//! ([`Previews::show`]). A pass not yet shown when the next one is in is skipped.
//!
//! The scene's own lightmaps are hidden under the preview, in its set
//! (`LightmapSet::shown_over`, their one owner), so a save meanwhile writes them,
//! never a preview's pages, and an edit (a despawn, a clear) reaches them. A
//! cancel or a failure puts them back and removes the preview pages
//! ([`Previews::restore`]); a finished bake's write replaces both, its clear of the
//! folder taking the preview pages with it.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError, TryLockError};
use std::thread::{self, JoinHandle};

use image::codecs::png::{CompressionType, FilterType, PngEncoder};
use image::{ExtendedColorType, ImageEncoder};

use super::job::Taken;
use super::lightmap_dir;
use super::write::{clear_pages, stems, write_png, PREVIEW};
use crate::scene::lighting::lightmap::{
    check_page_count, encode_directions, encode_masks, encode_texels, pack, BakeProgress, Lightmap,
    LightmapEntry, LightmapSet, Pass, MAX_PAGES,
};
use crate::scene::Scene;

/// The newest pass written, handed from the encoder to the UI thread.
type Latest = Arc<Mutex<Option<(Pass, LightmapSet)>>>;

/// One bake's previews, as the UI thread sees them: the newest pass written.
pub(super) struct Previews {
    latest: Latest,
    dir: PathBuf,
}

impl Previews {
    /// Previews written beside `scene_path`.
    pub fn new(scene_path: &str) -> Self {
        Self {
            latest: Latest::default(),
            dir: lightmap_dir(scene_path),
        }
    }

    /// Show the newest pass on `scene`, if one came in since the last call, and
    /// say which. The scene's own lightmaps are hidden under it the first time.
    /// A frame that finds the encoder writing a pass waits for the next.
    pub fn show(&mut self, scene: &mut Scene) -> Result<Option<Pass>, String> {
        let mut latest = match self.latest.try_lock() {
            Ok(latest) => latest,
            Err(TryLockError::Poisoned(e)) => e.into_inner(),
            Err(TryLockError::WouldBlock) => return Ok(None),
        };
        let Some((pass, mut set)) = latest.take() else {
            return Ok(None);
        };
        let shown = &mut scene.lightmaps;
        let own = shown.shown_over.take();
        set.shown_over = Some(own.unwrap_or_else(|| Box::new((**shown).clone())));
        let keep = stems(&set);
        scene.lightmaps = set.into();
        // The pass shown before this one; the lock keeps the encoder from
        // writing the next meanwhile.
        clear_pages(&self.dir, PREVIEW, &keep)?;
        Ok(Some(pass))
    }

    /// Put the scene's own lightmaps back, if a preview is shown, and remove the
    /// preview pages.
    pub fn restore(&mut self, scene: &mut Scene) {
        if let Some(own) = scene.lightmaps.shown_over.take() {
            scene.lightmaps = (*own).into();
        }
        self.clear();
    }

    /// Remove the preview pages; the folder may hold none. Once the bake is
    /// cancelled, the encoder writes none after this.
    pub fn clear(&self) {
        let _writing = lock(&self.latest);
        if let Err(e) = clear_pages(&self.dir, PREVIEW, &BTreeSet::new()) {
            log::warn!("[LightmapBake] remove the preview lightmaps: {e}");
        }
    }
}

/// The pass waiting for the encoder (a newer one replaces it), and whether the
/// bake is over.
#[derive(Default)]
struct Queue {
    next: Mutex<(Option<(Pass, Vec<Lightmap>)>, bool)>,
    woken: Condvar,
}

/// The thread writing a bake's preview passes, beside its worker.
pub(super) struct Encoder {
    queue: Arc<Queue>,
    thread: Option<JoinHandle<()>>,
}

impl Encoder {
    /// Start writing the passes [`Self::put`] hands in as `previews`' pages of at
    /// most `page` texels, labelled with what the bake `taken`; it stops once
    /// `progress` is cancelled.
    pub fn start(
        previews: &Previews,
        page: u32,
        taken: Taken,
        progress: Arc<BakeProgress>,
    ) -> Result<Self, String> {
        let queue = Arc::new(Queue::default());
        let (from, latest, dir) = (queue.clone(), previews.latest.clone(), previews.dir.clone());
        let thread = thread::Builder::new()
            .name("lightmap-preview".into())
            .spawn(move || {
                while let Some((pass, maps)) = from.take() {
                    // Written under the lock, so the UI never clears a page being
                    // written, nor writes one after a cancel's clear.
                    let mut latest = lock(&latest);
                    if progress.is_cancelled() {
                        return;
                    }
                    match write_preview(&dir, &maps, page, &taken) {
                        Ok(set) => *latest = Some((pass, set)),
                        Err(e) => log::warn!("[LightmapBake] write a preview pass: {e}"),
                    }
                }
            })
            .map_err(|e| format!("start the preview writer: {e}"))?;
        Ok(Self {
            queue,
            thread: Some(thread),
        })
    }

    /// Hand `pass`'s lightmaps `maps` in, replacing a pass still waiting.
    pub fn put(&self, pass: Pass, maps: Vec<Lightmap>) {
        lock(&self.queue.next).0 = Some((pass, maps));
        self.queue.woken.notify_one();
    }
}

impl Drop for Encoder {
    /// The bake is over: let the pass being written finish, drop a waiting one,
    /// and wait for the thread, so no preview page lands after the bake.
    fn drop(&mut self) {
        *lock(&self.queue.next) = (None, true);
        self.queue.woken.notify_one();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Queue {
    /// The next pass to write, waiting for one; `None` once the bake is over.
    fn take(&self) -> Option<(Pass, Vec<Lightmap>)> {
        let mut next = lock(&self.next);
        loop {
            if let Some(pass) = next.0.take() {
                return Some(pass);
            }
            if next.1 {
                return None;
            }
            next = self
                .woken
                .wait(next)
                .unwrap_or_else(PoisonError::into_inner);
        }
    }
}

/// `maps` packed into pages of at most `page` texels and written into `dir` as
/// preview pages, each an uncompressed PNG of one level (no BC encode, no mips),
/// and the set that points at them.
fn write_preview(
    dir: &Path,
    maps: &[Lightmap],
    page: u32,
    (lights, shadowmask): &Taken,
) -> Result<LightmapSet, String> {
    check_page_count(maps, page, MAX_PAGES)?;
    let atlas = pack(maps, page);
    let size = atlas.page_size;
    std::fs::create_dir_all(dir).map_err(|e| format!("lightmap folder: {e}"))?;
    let mut set = LightmapSet::default();
    let write =
        |kind: &str, k: usize, rgba: Vec<u8>| write_png(dir, kind, k, &raw_png(size, &rgba)?);
    for (k, texels) in atlas.pages.iter().enumerate() {
        set.pages.push(write(PREVIEW, k, encode_texels(texels))?);
    }
    for (k, dirs) in atlas.directions.iter().enumerate() {
        let kind = format!("{PREVIEW}_dir");
        set.directions
            .push(write(&kind, k, encode_directions(dirs))?);
    }
    for (k, mask) in atlas.masks.iter().enumerate() {
        let kind = format!("{PREVIEW}_mask");
        set.masks.push(write(&kind, k, encode_masks(mask))?);
    }
    let entry = |&(entity, page, scale_offset): &(u32, u32, [f32; 4])| LightmapEntry {
        entity,
        page,
        scale_offset,
    };
    set.extend(atlas.placements.iter().map(entry));
    set.lights = lights.clone();
    set.channels = shadowmask.channels.clone();
    set.fell_back = shadowmask.fell_back.clone();
    Ok(set)
}

/// A `size`² RGBA8 page as an uncompressed PNG's bytes: the cheapest the loader
/// reads.
fn raw_png(size: u32, rgba: &[u8]) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::with_capacity(rgba.len() + rgba.len() / 1024 + 1024);
    let png = PngEncoder::new_with_quality(
        &mut bytes,
        CompressionType::Uncompressed,
        FilterType::NoFilter,
    );
    png.write_image(rgba, size, size, ExtendedColorType::Rgba8)
        .map_err(|e| format!("encode a preview page: {e}"))?;
    Ok(bytes)
}

fn lock<V>(m: &Mutex<V>) -> MutexGuard<'_, V> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}
