//! Making an image's compressed cache current (#1574), at import and never at a
//! draw: one image ([`refresh`]), every image under a folder ([`refresh_all`],
//! Play's refresh and an export), or a queue a worker thread drains ([`queue`],
//! the editor). Each encode holds the cache's lock (`convert::lock_cache`), so the
//! worker and a Play refresh never make one cache twice at once.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Sender};
use std::sync::Mutex;

use super::{cache, cache_file, encode, fit, format_of, shrink, TextureKind};
use crate::asset::{convert, is_image_path, sidecar};

/// Make the compressed cache of the image at `source` current, encoding it when it
/// is missing or stale. Returns whether it encoded; `false` too when the image is
/// left uncompressed (see the module doc), which is found from its header without
/// decoding it. `progress` follows the image's name in the console line.
pub fn refresh(source: &Path, progress: &str) -> Result<bool, String> {
    let settings = sidecar::texture(source);
    if format_of(&settings).is_none() {
        return Ok(false);
    }
    let named = |e: &dyn std::fmt::Display| format!("{}: {e}", source.display());
    let out = cache_file(source)?;
    let bytes = std::fs::read(source).map_err(|e| named(&e))?;
    let stamp = cache::stamp(&bytes, &settings);
    if cache::read(&out, stamp).is_some() {
        return Ok(false);
    }
    let (w, h) = image::ImageReader::new(std::io::Cursor::new(&bytes))
        .with_guessed_format()
        .map_err(|e| named(&e))?
        .into_dimensions()
        .map_err(|e| named(&e))?;
    let (w, h) = shrink::fitted_size(w, h, settings.max_size);
    if w % 4 != 0 || h % 4 != 0 {
        return Ok(false);
    }
    if let Some(dir) = out.parent() {
        std::fs::create_dir_all(dir).map_err(|e| named(&e))?;
    }
    // Held until this returns; checked again under it, since whoever held it
    // may have just made this cache.
    let _lock = convert::lock_cache(&out);
    if cache::read(&out, stamp).is_some() {
        return Ok(false);
    }
    log::info!("[Import] compressing {}{progress}", source.display());
    let image = image::load_from_memory(&bytes).map_err(|e| named(&e))?;
    let kind = TextureKind::of_type(settings.texture_type);
    let fitted = fit(image, settings.max_size, kind);
    let Some(made) = encode::encode(&fitted, &settings) else {
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

/// [`refresh`] every image under `root`, in order, on this thread: what Play waits
/// for before it starts, as it waits for models (`convert::refresh_sources`), and
/// what an export runs before it copies the caches. Each one re-made is noted in
/// `core::fs::written`, so a renderer holding it uncompressed swaps it. Returns how
/// many it encoded; a failure is warned of, the rest still run, and the failures
/// are the `Err`, one per line. A standalone game (`convert::is_standalone`)
/// encodes nothing, as Unity's player imports nothing: it plays from the caches
/// its export shipped, and an image without one uploads uncompressed.
pub fn refresh_all(root: &Path) -> Result<usize, String> {
    if convert::is_standalone() {
        return Ok(0);
    }
    let images = images(root);
    let (mut encoded, mut failed) = (0, Vec::new());
    for (i, image) in images.iter().enumerate() {
        match refresh(image, &format!(" ({} of {} images)", i + 1, images.len())) {
            Ok(true) => {
                crate::core::fs::written::note_written(&image.to_string_lossy());
                encoded += 1;
            }
            Ok(false) => {}
            Err(e) => {
                log::warn!("[Import] {e}");
                failed.push(e);
            }
        }
    }
    match failed.is_empty() {
        true => Ok(encoded),
        false => Err(failed.join("\n")),
    }
}

/// The worker's inbox, started by the first [`queue`].
static WORKER: Mutex<Option<Sender<PathBuf>>> = Mutex::new(None);

/// [`refresh`] each of `sources` on a worker thread, in order, so the editor stays
/// responsive while it encodes; each one re-made is noted in `core::fs::written`,
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
                match refresh(&source, "") {
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
