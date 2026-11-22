use std::path::{Path, PathBuf};

use super::encode::{self, Cores};
use super::{cache, cache_file, fit, format_of, keeps_deep, open, reader, shrink, TextureKind};
use crate::convert::texture_ref::extracted;
use crate::convert::{self, Seen};
use crate::lightmap_page::is_baked;
use crate::sidecar::TextureSettings;
use crate::{is_image_path, sidecar};
use rusty_core::hash::content;

pub fn refresh(source: &Path, progress: &str) -> Result<bool, String> {
    refresh_on(source, progress, Cores::All)
}

struct Due {
    settings: crate::sidecar::TextureSettings,
    out: PathBuf,
    /// The image's size on disk, before `max_size` fits it.
    size: (u32, u32),
}

pub(super) enum Uncached {
    Left(&'static str),
    Unreadable(String),
}

pub(super) struct Cached {
    pub format: encode::BlockFormat,
    pub size: (u32, u32),
}

/// Shared by an encode ([`due`]) and the memory tests' blank cache (`cache::write_blank`), so
/// neither makes a cache the other would not.
pub(super) fn cached_as(source: &Path, settings: &TextureSettings) -> Result<Cached, Uncached> {
    use image::ImageDecoder;
    let format = format_of(settings).ok_or(Uncached::Left("compression is off"))?;
    let header = open(source)
        .and_then(|r| r.into_decoder())
        .map_err(|e| Uncached::Unreadable(format!("{}: {e}", source.display())))?;
    if keeps_deep(settings, header.color_type()) {
        return Err(Uncached::Left("a 16-bit normal map is kept uncompressed"));
    }
    let size = header.dimensions();
    let (w, h) = shrink::fitted_size(size.0, size.1, settings.max_size);
    if w % 4 != 0 || h % 4 != 0 {
        return Err(Uncached::Left("its fitted size is not a multiple of 4"));
    }
    Ok(Cached { format, size })
}

fn due(source: &Path) -> Result<Option<Due>, String> {
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
    match cached_as(source, &settings) {
        Ok(Cached { size, .. }) => Ok(Some(Due {
            settings,
            out,
            size,
        })),
        Err(Uncached::Left(_)) => Ok(None),
        Err(Uncached::Unreadable(e)) => Err(e),
    }
}

/// What a timed bench refuses to start with (`make bench-ab`): decoded uploads would skew its
/// memory and load.
pub fn pending(root: &Path) -> Vec<PathBuf> {
    let due = |image: &PathBuf| due(image).is_ok_and(|d| d.is_some());
    images(root).into_iter().filter(due).collect()
}

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
    // Checked again under the lock: whoever held it may have just made this cache.
    let _lock = convert::lock_cache(&out);
    let bytes = std::fs::read(source).map_err(|e| named(&e))?;
    // From the bytes encoded, not `due`'s remembered digest, so a file rewritten meanwhile is
    // stamped right.
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

/// `Renderer::uncached_textures`' test, so its audit and the import agree on which files get a
/// cache.
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

fn is_compressible(path: &Path) -> bool {
    is_image_path(path) && !is_baked(path)
}

#[derive(Debug, Default, PartialEq)]
pub struct Refreshed {
    pub encoded: usize,
    pub failed: Vec<(PathBuf, String)>,
    pub removed: usize,
}

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
