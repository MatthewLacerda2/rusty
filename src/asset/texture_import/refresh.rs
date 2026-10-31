//! Making an image's compressed cache current (#1574), at import and never at a
//! draw: one image ([`refresh`]), every image under a folder ([`refresh_all`],
//! Play's refresh, an export and `Assets.Refresh`), or a queue a worker thread drains ([`queue`],
//! the editor). Each encode holds the cache's lock (`convert::lock_cache`), so the
//! worker and a Play refresh never make one cache twice at once.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Sender};
use std::sync::Mutex;

use super::encode::{self, Cores};
use super::{cache, cache_file, fit, format_of, open, reader, shrink, TextureKind};
use crate::asset::convert::{self, Seen};
use crate::asset::{is_image_path, sidecar};
use crate::core::fs::written::Stale;
use crate::core::hash::content;

/// Make the compressed cache of the image at `source` current, encoding it on
/// every core when it is missing or stale. Returns whether it encoded; `false` too
/// when the image is left uncompressed (see the module doc), which is found from
/// its header without decoding it. `progress` follows the image's name in the
/// console line.
pub fn refresh(source: &Path, progress: &str) -> Result<bool, String> {
    refresh_on(source, progress, Cores::All)
}

/// [`refresh`] on `cores`. Whether the cache is current costs a `stat` of the
/// source (its digest is remembered by length and mtime, `convert::source_digest`)
/// and a read of the cache's head; only an encode reads either whole.
fn refresh_on(source: &Path, progress: &str, cores: Cores) -> Result<bool, String> {
    let settings = sidecar::texture(source);
    if format_of(&settings).is_none() {
        return Ok(false);
    }
    let named = |e: &dyn std::fmt::Display| format!("{}: {e}", source.display());
    let out = cache_file(source)?;
    let digest = match convert::source_digest(source) {
        Seen::Read(digest) => digest,
        Seen::Absent => return Err(named(&"no such file")),
        Seen::Unreadable => return Err(named(&"cannot be read")),
    };
    if cache::is_current(&out, cache::stamp(digest, &settings)) {
        return Ok(false);
    }
    let (w, h) = open(source)
        .and_then(|r| r.into_dimensions())
        .map_err(|e| named(&e))?;
    let (w, h) = shrink::fitted_size(w, h, settings.max_size);
    if w % 4 != 0 || h % 4 != 0 {
        return Ok(false);
    }
    if let Some(dir) = out.parent() {
        std::fs::create_dir_all(dir).map_err(|e| named(&e))?;
    }
    // Held until this returns; checked again under it, since whoever held it
    // may have just made this cache. The stamp is made from the bytes encoded.
    let _lock = convert::lock_cache(&out);
    let bytes = std::fs::read(source).map_err(|e| named(&e))?;
    let stamp = cache::stamp(content(&bytes), &settings);
    if cache::is_current(&out, stamp) {
        return Ok(false);
    }
    log::info!("[Import] compressing {}{progress}", source.display());
    let image = reader(source, std::io::Cursor::new(&bytes))
        .and_then(|r| r.decode())
        .map_err(|e| named(&e))?;
    let kind = TextureKind::of_settings(&settings);
    let fitted = fit(image, settings.max_size, kind);
    let Some(made) = encode::encode(&fitted, &settings, cores) else {
        return Ok(false);
    };
    cache::write(&out, stamp, &made).map_err(|e| named(&e))?;
    Ok(true)
}

/// Every image under `root` ([`crate::asset::IMAGE_EXTENSIONS`]), sorted: what a
/// refresh compresses and an export ships the caches of.
pub fn images(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    crate::asset::manifest::walk(root, &mut |path| {
        if is_image_path(path) {
            found.push(path.to_path_buf());
        }
    });
    found.sort();
    found
}

/// What a [`refresh_all`] did: how many images it encoded, and each one it could
/// not, with why.
#[derive(Debug, Default, PartialEq)]
pub struct Refreshed {
    pub encoded: usize,
    pub failed: Vec<(PathBuf, String)>,
}

/// [`refresh`] every image under `root`, in order, on this thread: what Play waits
/// for before it starts, as it waits for models (`convert::refresh_sources`), and
/// what an export runs before it copies the caches. Each one re-made is noted in
/// `core::fs::written`, so a renderer holding it uncompressed swaps it. An image
/// that fails is warned of and the rest still run: it draws (and ships)
/// uncompressed, as Unity builds with an import error logged. `Assets.Refresh`
/// runs it on request. Nothing while imports are refused (Play,
/// `convert::conversion_allowed`).
pub fn refresh_all(root: &Path) -> Refreshed {
    let mut done = Refreshed::default();
    if !convert::conversion_allowed() {
        return done;
    }
    let images = images(root);
    for (i, image) in images.iter().enumerate() {
        match refresh(image, &format!(" ({} of {} images)", i + 1, images.len())) {
            Ok(true) => {
                crate::core::fs::written::note_written(&image.to_string_lossy());
                done.encoded += 1;
            }
            Ok(false) => {}
            Err(e) => {
                log::warn!("[Import] {e}; it stays uncompressed");
                done.failed.push((image.clone(), e));
            }
        }
    }
    done
}

/// [`queue`] every image under `root` that `changed` names, or whose sidecar it
/// names: the editor watch's refresh (`convert::refresh_changed`).
pub fn queue_changed(root: &Path, changed: &Stale) {
    if changed.is_empty() {
        return;
    }
    let names = |path: &Path| changed.matches(&path.to_string_lossy());
    let touched = images(root)
        .into_iter()
        .filter(|image| names(image) || names(&sidecar::meta_path(image)));
    queue(touched.collect());
}

/// The worker's inbox, started by the first [`queue`].
static WORKER: Mutex<Option<Sender<PathBuf>>> = Mutex::new(None);

/// [`refresh`] each of `sources` on a worker thread, in order, on half the cores,
/// so the editor stays responsive while it encodes; each one re-made is noted in `core::fs::written`,
/// so the renderer swaps its uncompressed upload for it. Nothing is queued while
/// imports are refused (Play, `convert::conversion_allowed`): Play's own refresh
/// ran before it, and a change seen during it is queued after Stop.
pub fn queue(sources: Vec<PathBuf>) {
    if sources.is_empty() || !convert::conversion_allowed() {
        return;
    }
    let mut worker = WORKER.lock().unwrap_or_else(|e| e.into_inner());
    let inbox = worker.get_or_insert_with(start);
    for source in sources {
        if inbox.send(source).is_err() {
            // The worker is gone (it panicked): the next queue starts another.
            log::warn!("[Import] the texture import worker stopped; textures stay uncompressed");
            *worker = None;
            return;
        }
    }
}

/// Start the worker: it refreshes what it is sent until the process ends.
fn start() -> Sender<PathBuf> {
    let (inbox, sent) = channel::<PathBuf>();
    let spawned = std::thread::Builder::new()
        .name("texture-import".into())
        .spawn(move || {
            for source in sent {
                match refresh_on(&source, "", Cores::Half) {
                    Ok(true) => {
                        let path = source.to_string_lossy();
                        crate::core::fs::written::note_written(&path);
                        log::info!("[Import] compressed {path}");
                    }
                    Ok(false) => {}
                    Err(e) => log::warn!("[Import] {e}"),
                }
            }
        });
    if let Err(e) = spawned {
        log::warn!("[Import] could not start the texture import worker: {e}");
    }
    inbox
}
