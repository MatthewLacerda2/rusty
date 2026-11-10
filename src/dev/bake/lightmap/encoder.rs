//! The thread writing a progressive bake's preview passes beside its worker
//! (#1970): it writes a pass's pages without holding the lock the UI shares, then
//! takes it only to hand the pass over and remove the pages nobody will show.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::thread::{self, JoinHandle};

use image::codecs::png::{CompressionType, FilterType, PngEncoder};
use image::{ExtendedColorType, ImageEncoder};

use super::job::Taken;
use super::preview::{lock, Previews, Shared};
use super::write::{clear_pages, stems, write_png};
use crate::scene::lighting::lightmap::{
    check_page_count, encode_directions, encode_masks, encode_texels, pack, BakeProgress, Lightmap,
    LightmapEntry, LightmapSet, Pass, PassSink, MAX_PAGES,
};

/// A pass's lightmaps, handed to the encoder.
type Handed = (Pass, Vec<Lightmap>);

/// The pass waiting for the encoder (a newer one replaces it), whether one is
/// being written, and whether the bake is over.
#[derive(Default)]
struct State {
    waiting: Option<Handed>,
    writing: bool,
    over: bool,
}

#[derive(Default)]
struct Queue {
    state: Mutex<State>,
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
        let (from, shared) = (queue.clone(), previews.latest.clone());
        let at = (previews.dir.clone(), previews.prefix.clone());
        let thread = thread::Builder::new()
            .name("lightmap-preview".into())
            .spawn(move || {
                while let Some((pass, maps)) = from.take() {
                    // Written without the lock, so the UI never waits on a write.
                    let written = write_preview(&at, &maps, page, &taken);
                    drop(maps);
                    let over = hand_over(&at, &shared, (pass, written), &progress);
                    from.written();
                    if over {
                        return;
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
        lock(&self.queue.state).waiting = Some((pass, maps));
        self.queue.woken.notify_one();
    }

    /// Whether a pass is waiting or being written: the bake builds no lightmaps
    /// for a preview while it is (#1970's review), so at most one pass's are
    /// alive beside the shown pages.
    pub fn busy(&self) -> bool {
        let state = lock(&self.queue.state);
        state.waiting.is_some() || state.writing
    }
}

impl PassSink for Encoder {
    fn wants(&self) -> bool {
        !self.busy()
    }

    fn pass(&mut self, pass: Pass, maps: Vec<Lightmap>) {
        self.put(pass, maps);
    }
}

/// Under the lock, make `written` the newest pass (unless the bake was
/// cancelled) and remove every preview page neither shown nor newest: the pass it
/// replaced before it was shown, a failed write's pages, or after a cancel its
/// own. Only this bake's pages (named from `prefix`): a writer finishing after
/// its bake was dropped never touches the next bake's. Whether the bake is over.
fn hand_over(
    (dir, prefix): &(PathBuf, String),
    shared: &Mutex<Shared>,
    (pass, written): (Pass, Result<LightmapSet, String>),
    progress: &BakeProgress,
) -> bool {
    let mut shared = lock(shared);
    let cancelled = progress.is_cancelled();
    match written {
        Ok(set) if !cancelled => shared.latest = Some((pass, set)),
        Ok(_) => {}
        Err(e) => log::warn!("[LightmapBake] write a preview pass: {e}"),
    }
    let mut keep = shared.shown.clone();
    keep.extend(
        shared
            .latest
            .as_ref()
            .map_or_else(BTreeSet::new, |(_, s)| stems(s)),
    );
    if let Err(e) = clear_pages(dir, prefix, &keep) {
        log::warn!("[LightmapBake] remove a preview page never shown: {e}");
    }
    cancelled
}

impl Drop for Encoder {
    /// The bake is over: let the pass being written finish, drop a waiting one,
    /// and wait for the thread, so no preview page lands after the bake.
    fn drop(&mut self) {
        let mut state = lock(&self.queue.state);
        (state.waiting, state.over) = (None, true);
        drop(state);
        self.queue.woken.notify_one();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Queue {
    /// The next pass to write, waiting for one; `None` once the bake is over.
    fn take(&self) -> Option<Handed> {
        let mut state = lock(&self.state);
        loop {
            if let Some(pass) = state.waiting.take() {
                state.writing = true;
                return Some(pass);
            }
            if state.over {
                return None;
            }
            state = self
                .woken
                .wait(state)
                .unwrap_or_else(PoisonError::into_inner);
        }
    }

    /// The pass taken is written (or dropped).
    fn written(&self) {
        lock(&self.state).writing = false;
    }
}

/// `maps` packed into pages of at most `page` texels and written into `dir` as
/// preview pages named from `prefix`, each an uncompressed PNG of one level (no
/// BC encode, no mips), and the set that points at them.
fn write_preview(
    (dir, prefix): &(PathBuf, String),
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
        set.pages.push(write(prefix, k, encode_texels(texels))?);
    }
    for (k, dirs) in atlas.directions.iter().enumerate() {
        let kind = format!("{prefix}_dir");
        set.directions
            .push(write(&kind, k, encode_directions(dirs))?);
    }
    for (k, mask) in atlas.masks.iter().enumerate() {
        let kind = format!("{prefix}_mask");
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
