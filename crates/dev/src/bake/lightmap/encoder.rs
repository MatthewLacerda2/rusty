use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::thread::{self, JoinHandle};

use image::codecs::png::{CompressionType, FilterType, PngEncoder};
use image::{ExtendedColorType, ImageEncoder};

use super::job::Taken;
use super::preview::{lock, Previews, Shared};
use super::write::{clear_pages, stems, write_png};
use rusty_lighting::lightmap::{
    check_page_count, encode_directions, encode_masks, encode_texels, packing, BakeProgress,
    Lightmap, Pass, PassSink, MAX_PAGES,
};
use rusty_scene::lighting::lightmap_set::{LightmapEntry, LightmapSet};

type Handed = (Pass, Vec<Lightmap>);

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

pub(super) struct Encoder {
    queue: Arc<Queue>,
    thread: Option<JoinHandle<()>>,
}

impl Encoder {
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
                    // Written without the shared lock, so the UI never waits on a write.
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

    pub fn put(&self, pass: Pass, maps: Vec<Lightmap>) {
        lock(&self.queue.state).waiting = Some((pass, maps));
        self.queue.woken.notify_one();
    }

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

    fn written(&self) {
        lock(&self.state).writing = false;
    }
}

fn write_preview(
    (dir, prefix): &(PathBuf, String),
    maps: &[Lightmap],
    page: u32,
    (lights, shadowmask): &Taken,
) -> Result<LightmapSet, String> {
    check_page_count(maps, page, MAX_PAGES)?;
    let layout = packing(maps, page);
    let size = layout.page_size;
    std::fs::create_dir_all(dir).map_err(|e| format!("lightmap folder: {e}"))?;
    let mut set = LightmapSet::default();
    let write =
        |kind: &str, k: usize, rgba: Vec<u8>| write_png(dir, kind, k, &raw_png(size, &rgba)?);
    for k in 0..layout.page_count {
        let page = layout.page(k, &|i| &maps[i]);
        set.pages
            .push(write(prefix, k, encode_texels(&page.texels))?);
        if let Some(dirs) = page.directions {
            let kind = format!("{prefix}_dir");
            set.directions
                .push(write(&kind, k, encode_directions(&dirs))?);
        }
        if let Some(mask) = page.masks {
            let kind = format!("{prefix}_mask");
            set.masks.push(write(&kind, k, encode_masks(&mask))?);
        }
    }
    let entry = |(entity, page, scale_offset): (u32, u32, [f32; 4])| LightmapEntry {
        entity,
        page,
        scale_offset,
    };
    set.extend(layout.placements(maps).into_iter().map(entry));
    set.lights = lights.keys().copied().collect();
    set.channels = shadowmask.channels.clone();
    set.fell_back = shadowmask.fell_back.clone();
    set.record_taken(lights);
    Ok(set)
}

fn raw_png(size: u32, rgba: &[u8]) -> Result<Vec<u8>, String> {
    // Bytes: the pixels, plus slack for the row filter bytes and the blocks' and chunks' headers.
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
