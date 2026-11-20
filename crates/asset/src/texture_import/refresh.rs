//! Making an image's compressed cache current (#1574), at import and never at a
//! draw: one image ([`refresh`]), every image under a folder ([`refresh_all`],
//! Play's refresh, an export and `Assets.Refresh`), or a queue a worker thread drains
//! (`background`, the editor). Each encode holds the cache's lock (`convert::lock_cache`), so the
//! worker and a Play refresh never make one cache twice at once.

use std::path::{Path, PathBuf};

use super::encode::{self, Cores};
use super::{cache, cache_file, fit, format_of, keeps_deep, open, reader, shrink, TextureKind};
use crate::convert::texture_ref::extracted;
use crate::convert::{self, Seen};
use crate::lightmap_page::is_baked;
use crate::{is_image_path, sidecar};
use rusty_core::hash::content;

/// Make the compressed cache of the image at `source` current, encoding it on
/// every core when it is missing or stale. Returns whether it encoded; `false` too
/// when the image is left uncompressed (see the module doc), which is found from
/// its header without decoding it. `progress` follows the image's name in the
/// console line.
pub fn refresh(source: &Path, progress: &str) -> Result<bool, String> {
    refresh_on(source, progress, Cores::All)
}

/// What an encode of one image is to make: the found half of [`refresh_on`].
struct Due {
    settings: crate::sidecar::TextureSettings,
    out: PathBuf,
    /// The image's size on disk, before `max_size` fits it.
    size: (u32, u32),
}

/// Whether the image at `source` is due an encode, from its sidecar, the cache's
/// head and its own header (never decoded): `None` when the cache is current or
/// the image is one left uncompressed (no block format, a deep normal map kept
/// at 16 bits, a fitted size not a multiple of 4).
fn due(source: &Path) -> Result<Option<Due>, String> {
    use image::ImageDecoder;
    let settings = sidecar::texture(source);
    if format_of(&settings).is_none() {
        return Ok(None);
    }
    let named = |e: &dyn std::fmt::Display| format!("{}: {e}", source.display());
    let out = cache_file(source)?;
    let digest = match convert::source_digest(source) {
        Seen::Read(digest) => digest,
        Seen::Absent => return Err(named(&"no such file")),
        Seen::Unreadable => return Err(named(&"cannot be read")),
    };
    if cache::is_current(&out, cache::stamp(digest, &settings)) {
        return Ok(None);
    }
    let header = open(source)
        .and_then(|r| r.into_decoder())
        .map_err(|e| named(&e))?;
    if keeps_deep(&settings, header.color_type()) {
        return Ok(None);
    }
    let size = header.dimensions();
    let (w, h) = shrink::fitted_size(size.0, size.1, settings.max_size);
    if w % 4 != 0 || h % 4 != 0 {
        return Ok(None);
    }
    Ok(Some(Due {
        settings,
        out,
        size,
    }))
}

/// The images under `root` an import would encode now ([`images`] whose cache is
/// missing or stale, the ones left uncompressed aside), read without encoding:
/// what a timed bench refuses to start with (`make bench-ab`), since each would
/// upload decoded and its scene's memory and load would not be the game's. An
/// image that cannot be read is not listed: no import makes its cache either.
pub fn pending(root: &Path) -> Vec<PathBuf> {
    let due = |image: &PathBuf| due(image).is_ok_and(|d| d.is_some());
    images(root).into_iter().filter(due).collect()
}

/// [`refresh`] on `cores`. Whether the cache is current costs a `stat` of the
/// source (its digest is remembered by length and mtime, `convert::source_digest`)
/// and a read of the cache's head; only an encode reads either whole.
pub(super) fn refresh_on(source: &Path, progress: &str, cores: Cores) -> Result<bool, String> {
    let Some(Due {
        settings,
        out,
        size,
    }) = due(source)?
    else {
        return Ok(false);
    };
    let named = |e: &dyn std::fmt::Display| format!("{}: {e}", source.display());
    shrink::warn_if_shrunk(source, size, settings.max_size);
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

/// Every image under `root` ([`crate::IMAGE_EXTENSIONS`]), and every texture
/// extracted from a model under it (`convert::texture_ref::extracted`, #2365),
/// sorted: what a refresh compresses and an export ships the caches of. A baked
/// lightmap page is not among them (`asset::lightmap_page::is_baked`, #1694): it
/// is not a colour texture, and the renderer loads the page the bake wrote. A
/// model's extracted textures are found from its cache as the last conversion
/// left it, which this never converts.
pub fn images(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    crate::manifest::walk(root, &mut |path| {
        if is_compressible(path) {
            found.push(path.to_path_buf());
        } else if convert::converter_for(path).is_some() {
            if let Ok(glb) = convert::cache_location(path) {
                found.extend(extracted::images(&glb).into_iter().filter(|i| !is_baked(i)));
            }
        }
    });
    found.sort();
    found
}

/// The image files under `root`, outside the import cache, sorted: [`images`]
/// without the textures models extracted, which the editor's watch never sees
/// change (it ignores the import cache; a conversion queues them, `background`).
pub(super) fn files(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    crate::manifest::walk(root, &mut |path| {
        if is_compressible(path) {
            found.push(path.to_path_buf());
        }
    });
    found.sort();
    found
}

/// Whether [`refresh_all`] on `root` visits `file`: an image under `root`
/// outside the import cache (`manifest::walk` leaves the cache out), or a texture
/// extracted from a model under `root`, and not a baked page ([`images`]). What an
/// audit of uploads asks (`Renderer::uncached_textures`), so the audit and the
/// import cannot disagree on which files an import makes a cache for. Paths are
/// compared made absolute, so `assets/a.png` and its absolute spelling are one
/// file. An extracted texture counts only while its model exists, as [`images`]
/// walks the models on disk. Its model is read from its project cache's path, or
/// found among the models under `root` outside a project (the temp folder's cache).
pub fn imports(root: &Path, file: &Path) -> bool {
    let absolute = |p: &Path| std::path::absolute(p).unwrap_or_else(|_| p.to_path_buf());
    let under = |p: &Path| absolute(p).starts_with(absolute(root));
    let visited = match extracted::glb_of(file) {
        Some(glb) => match extracted::model_source(file) {
            Some(model) => under(&model) && model.is_file(),
            None => convert::convertible_sources(root).iter().any(|model| {
                convert::cache_location(model).is_ok_and(|g| absolute(&g) == absolute(&glb))
            }),
        },
        None => under(file) && !convert::is_import_cache(file),
    };
    visited && is_compressible(file)
}

/// The per-file half of [`images`] and [`imports`]: an image, not a baked page.
fn is_compressible(path: &Path) -> bool {
    is_image_path(path) && !is_baked(path)
}

/// What a [`refresh_all`] did: how many images it encoded, each one it could
/// not, with why, and how many orphaned caches it deleted (`orphans`, #1694).
#[derive(Debug, Default, PartialEq)]
pub struct Refreshed {
    pub encoded: usize,
    pub failed: Vec<(PathBuf, String)>,
    pub removed: usize,
}

/// [`refresh`] every image under `root`, in order, on this thread: what Play waits
/// for before it starts, as it waits for models (`convert::refresh_sources`), and
/// what an export runs before it copies the caches. Each one re-made is noted in
/// `core::fs::written`, so a renderer holding it uncompressed swaps it. An image
/// that fails is warned of and the rest still run: it draws (and ships)
/// uncompressed, as Unity builds with an import error logged. Last, the caches
/// whose source is gone, is a baked lightmap page or is a normal map kept at 16
/// bits are deleted (`orphans`, #1694). `Assets.Refresh` runs it on request. Nothing while imports
/// are refused (Play, `convert::conversion_allowed`).
pub fn refresh_all(root: &Path) -> Refreshed {
    let mut done = Refreshed::default();
    if !convert::conversion_allowed() {
        return done;
    }
    let images = images(root);
    for (i, image) in images.iter().enumerate() {
        match refresh(image, &format!(" ({} of {} images)", i + 1, images.len())) {
            Ok(true) => {
                rusty_core::fs::written::note_written(&image.to_string_lossy());
                done.encoded += 1;
            }
            Ok(false) => {}
            Err(e) => {
                log::warn!("[Import] {e}; it stays uncompressed");
                done.failed.push((image.clone(), e));
            }
        }
    }
    done.removed = super::orphans::collect(root);
    done
}

#[cfg(test)]
#[path = "pending_tests.rs"]
mod pending_tests;
