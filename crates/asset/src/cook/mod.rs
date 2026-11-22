//! What an import derives from a parsed model, computed once and kept on disk:
//! the unwrap for Generate Lightmap UVs, then clusters (last: they reorder indices),
//! with the parse generating tangents. All are pure functions of the parsed files and
//! geometry settings; sub-meshes run on threads ([`par`]) and join in document order,
//! so the result is bit-identical to one thread.
//!
//! Kept as `<source>.cooked.glb` in the import cache ([`SUFFIX`]); a model outside an
//! open project is cooked in memory ([`with_writes`] opts a test in). A missing, stale
//! or malformed file is a miss. Only authoring builds write ([`WRITES`]), never Play
//! or a standalone game.
//!
//! stored-format: the `<source>.cooked.glb` file of the import cache

pub mod file;
pub mod par;
pub mod stamp;
pub mod sub;

use std::cell::Cell;
use std::path::{Path, PathBuf};

use super::mesh_data::ImportedAsset;
use super::tangents::Missing;
use super::{clusters, convert, lightmap_uv, scale_factor, ImportError, ImportSettings};
use sub::Cooked;

pub const SUFFIX: &str = ".cooked.glb";

pub const WRITES: bool = cfg!(any(feature = "editor", feature = "dev"));

thread_local! {
    static COOKS: Cell<usize> = const { Cell::new(0) };
    static WRITTEN: Cell<usize> = const { Cell::new(0) };
    static PIN_WRITES: Cell<Option<bool>> = const { Cell::new(None) };
}

pub fn cooks() -> usize {
    COOKS.get()
}

pub fn written() -> usize {
    WRITTEN.get()
}

pub fn cooked_path(source: &Path) -> Option<PathBuf> {
    match PIN_WRITES.get() {
        Some(_) => convert::cache_location_as(source, SUFFIX).ok(),
        None => convert::project_cache_as(source, SUFFIX).ok()?,
    }
}

/// Whether `path` has a cooked file stamped with what it is now, told without parsing.
/// Only the stamp is read, so a read may still refuse it (a parser change without a
/// version bump).
pub fn is_stamped(path: &Path) -> bool {
    let read = match convert::converter_for(path) {
        Some(_) => convert::cache_location(path).ok().filter(|r| r.is_file()),
        None => Some(path.to_path_buf()),
    };
    let Some(read) = read else {
        return false;
    };
    let at = stamped_at(path, &read, &crate::settings_of(path));
    at.is_some_and(|(file, stamp)| file::stamped(&file, stamp))
}

pub fn pending(root: &Path) -> Vec<PathBuf> {
    let mut models = Vec::new();
    crate::manifest::walk(root, &mut |path| {
        let placed = crate::is_model_path(path) && cooked_path(path).is_some();
        if placed && !is_stamped(path) {
            models.push(path.to_path_buf());
        }
    });
    models.sort();
    models
}

fn stamped_at(path: &Path, read: &Path, settings: &ImportSettings) -> Option<(PathBuf, u64)> {
    cooked_path(path).and_then(|file| Some((file, stamp::stamp(read, settings)?)))
}

pub(crate) fn import(
    path: &Path,
    read: &Path,
    settings: &ImportSettings,
    parse: impl Fn(Missing) -> Result<ImportedAsset, ImportError>,
) -> Result<ImportedAsset, ImportError> {
    let scale = |asset: &mut ImportedAsset| {
        scale_factor::apply(asset, scale_factor::sanitize(settings.scale, 1.0));
    };
    let at = stamped_at(path, read, settings);
    if let Some(cooked) = at
        .as_ref()
        .and_then(|(file, stamp)| read_cooked(file, *stamp))
    {
        let mut asset = parse(Missing::Placeholder)?;
        scale(&mut asset);
        let laid = lay(&mut asset, cooked);
        match laid {
            Ok(()) => return Ok(asset),
            Err(why) => log::warn!("[Import] {}: {why}; cooking it again", path.display()),
        }
    }
    let mut asset = parse(Missing::Generate)?;
    scale(&mut asset);
    let at = at.filter(|_| writes());
    let parses: Vec<u64> = match at {
        Some(_) => {
            let subs: Vec<_> = asset.sub_meshes.iter().collect();
            par::map(subs, |s| s.indices.len() / 3, sub::digest)
        }
        None => Vec::new(),
    };
    let sources = derive(&mut asset, settings);
    COOKS.set(COOKS.get() + 1);
    if let Some((file, stamp)) = at {
        let subs: Vec<Cooked> = (asset.sub_meshes.iter().zip(parses).zip(sources))
            .map(|((s, parse), source)| Cooked::of(s, parse, source))
            .collect();
        if write(&file, &file::encode(stamp, &subs)) {
            WRITTEN.set(WRITTEN.get() + 1);
        }
    }
    Ok(asset)
}

pub(crate) fn derive(
    asset: &mut ImportedAsset,
    settings: &ImportSettings,
) -> Vec<Option<Vec<u32>>> {
    let subs: Vec<_> = asset.sub_meshes.iter_mut().collect();
    par::map(
        subs,
        |sub| sub.indices.len() / 3,
        |sub| {
            let source = match settings.lightmap_uvs.generate {
                true => lightmap_uv::apply(sub, &settings.lightmap_uvs),
                false => None,
            };
            clusters::apply(sub);
            source
        },
    )
}

fn read_cooked(file: &Path, stamp: u64) -> Option<Vec<Cooked>> {
    let bytes = std::fs::read(file).ok()?;
    match file::parse(&bytes, stamp) {
        Ok(subs) => Some(subs),
        Err(why) if why.starts_with(file::STALE) => None,
        Err(why) => {
            log::warn!(
                "[Import] {}: {why}; cooking its model again",
                file.display()
            );
            None
        }
    }
}

fn lay(asset: &mut ImportedAsset, cooked: Vec<Cooked>) -> Result<(), String> {
    if cooked.len() != asset.sub_meshes.len() {
        return Err(format!(
            "{} cooked sub-meshes for {} parsed",
            cooked.len(),
            asset.sub_meshes.len()
        ));
    }
    let pairs: Vec<_> = asset.sub_meshes.iter_mut().zip(cooked).collect();
    let laid = par::map(pairs, |(s, _)| s.indices.len() / 3, |(s, c)| c.apply(s));
    laid.into_iter().collect()
}

pub(crate) fn writes() -> bool {
    let authoring = convert::conversion_allowed() && !convert::gate::is_standalone();
    PIN_WRITES.get().unwrap_or(WRITES) && authoring
}

fn write(file: &Path, bytes: &[u8]) -> bool {
    let written = file
        .parent()
        .map_or(Ok(()), std::fs::create_dir_all)
        // lint: allow(asset-write) a cache of values the import just computed; read only when a model is parsed
        .and_then(|()| rusty_core::fs::write_atomic(file, bytes));
    if let Err(e) = &written {
        log::warn!("[Import] cannot write {}: {e}", file.display());
    }
    written.is_ok()
}

/// Run `f` with cooked files written (`true`) or not on this thread, whatever the
/// build. A test hook: public only for the `dev` crate's tests.
#[doc(hidden)]
pub fn with_writes<R>(on: bool, f: impl FnOnce() -> R) -> R {
    struct Restore(Option<bool>);
    impl Drop for Restore {
        fn drop(&mut self) {
            PIN_WRITES.set(self.0);
        }
    }
    let _restore = Restore(PIN_WRITES.replace(Some(on)));
    f()
}

#[cfg(test)]
mod cache_tests;
#[cfg(test)]
mod file_tests;
#[cfg(test)]
pub(crate) mod fixture;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod version;
