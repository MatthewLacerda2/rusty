//! crates/asset/src/cook/ — what an import derives from a parsed model, computed
//! once and kept on disk (#2332).
//!
//! **The steps.** After the parse and the Scale Factor (`scale_factor`), each
//! sub-mesh gets Generate Lightmap UVs' unwrap (`lightmap_uv`, when its sidecar
//! ticks it) and then its clusters (`clusters`, last, since they reorder the
//! indices every step before settled); the parse itself generates the tangents a
//! glTF or `.obj` lacks (`tangents`). Those three are most of a big model's
//! import, and all are pure functions of (the files parsed, the geometry
//! settings). The sub-meshes run on threads ([`par`]), joined in document order,
//! so the asset is the same value, bit for bit, as one thread makes; the parse
//! spreads its meshes the same way.
//!
//! **The cooked file.** What the steps computed is kept beside the model's other
//! import-cache files, at `convert::cache_location_as(<source>, `[`SUFFIX`]`)`:
//! in an open project `assets/generated/imported/<source>.cooked.glb` (shipped by
//! an export). A model outside an open project has none: it is cooked in memory
//! each time, so nothing piles up in the system temp folder (a test opts in with
//! [`with_writes`], which keeps it there). It is a glTF ([`mod@file`]) holding
//! each sub-mesh's tangents, lightmap UVs, its cluster-ordered indices and its
//! cluster table ([`sub`]), stamped by what it was made from ([`stamp`]). An
//! import whose stamp matches parses the model without generating tangents and
//! lays the cooked values onto it: it computes none of the three, and the result
//! equals a fresh cook bit for bit, so a baked lightmap fits a cooked load as it
//! fits a fresh one. A missing, stale or malformed file is a miss: the model is
//! cooked as before ([`cooks`] counts those).
//!
//! **Written** on a miss by an authoring build ([`WRITES`]: the editor or `dev`)
//! while conversion is allowed (edit mode, never Play) and never by a standalone
//! game (`convert::mark_standalone`: the player, or a dev harness such as
//! `bench-scenes`, which reads a project and writes nothing into it), as it
//! compresses no texture. Those read cooked files, never write one, and cook in
//! memory on a miss; `make import`, `Assets.Refresh` and the editor write them.
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

/// What a cooked file's name adds to its model source's.
pub const SUFFIX: &str = ".cooked.glb";

/// Whether this build writes cooked files: an authoring one.
pub const WRITES: bool = cfg!(any(feature = "editor", feature = "dev"));

thread_local! {
    static COOKS: Cell<usize> = const { Cell::new(0) };
    static WRITTEN: Cell<usize> = const { Cell::new(0) };
    static PIN_WRITES: Cell<Option<bool>> = const { Cell::new(None) };
}

/// How many models this thread has cooked (run the steps for, a cooked file
/// found current not counted): what tells a warm load from a cold one.
pub fn cooks() -> usize {
    COOKS.get()
}

/// How many cooked files this thread has written: what `make import` reports.
pub fn written() -> usize {
    WRITTEN.get()
}

/// Where the cooked file of the model source `source` lives: in its project's
/// import cache, and nowhere for a model outside an open project, unless
/// [`with_writes`] pinned this thread (the system temp folder then).
pub fn cooked_path(source: &Path) -> Option<PathBuf> {
    match PIN_WRITES.get() {
        Some(_) => convert::cache_location_as(source, SUFFIX).ok(),
        None => convert::project_cache_as(source, SUFFIX).ok()?,
    }
}

/// Whether the model source `path` has a cooked file stamped with what it is now
/// ([`stamp`]), told without parsing the model or converting a converted source
/// (the cached glTF as it stands is what a standalone load reads): what a bench
/// asks before it runs (`bench-scenes --pending-imports`). Only the stamp is
/// read, so a file it calls stamped may still be refused by the read a load
/// makes (a parser change without a version bump, a cut-short BIN): the import
/// pass decides by that read (`import::cook_models`), and a bench notes a scene
/// whose load cooked anyway.
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

/// The model files under `root`, sorted, with no cooked file [`is_stamped`] with
/// what they are now: what an import would cook, told without parsing any. A
/// model outside an open project (no place for a cooked file) is not listed.
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

/// The cooked file of the model at `path` (parsed from `read`) and the stamp it
/// must carry, when it has one: the files are hashed only then.
fn stamped_at(path: &Path, read: &Path, settings: &ImportSettings) -> Option<(PathBuf, u64)> {
    cooked_path(path).and_then(|file| Some((file, stamp::stamp(read, settings)?)))
}

/// The model at `path` (parsed from `read`) with its `settings`' Scale Factor and
/// cooked values: read from its cooked file when that is current, cooked
/// otherwise (and written, by an authoring build). `parse` parses it, doing for a
/// missing tangent what it is told.
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

/// Run the per-sub-mesh steps on `asset`, already scaled, with `settings`:
/// returns, per sub-mesh, the parsed vertex each final one copies when the unwrap
/// split them.
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

/// The cooked sub-meshes in `file` when it was cooked under `stamp`: none when it
/// is absent or stale, and none with a warning when it will not parse.
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

/// Lay `cooked` onto `asset`'s sub-meshes, one to one in order, or say why it
/// does not fit (`asset` is then to be parsed again).
fn lay(asset: &mut ImportedAsset, cooked: Vec<Cooked>) -> Result<(), String> {
    if cooked.len() != asset.sub_meshes.len() {
        return Err(format!(
            "{} cooked sub-meshes for {} parsed",
            cooked.len(),
            asset.sub_meshes.len()
        ));
    }
    let pairs: Vec<_> = asset.sub_meshes.iter_mut().zip(cooked).collect();
    // The parse digest each check takes is spread as the cook's steps are.
    let laid = par::map(pairs, |(s, _)| s.indices.len() / 3, |(s, c)| c.apply(s));
    laid.into_iter().collect()
}

/// Whether a cook on this thread is written to disk ([module docs](self)).
pub(crate) fn writes() -> bool {
    let authoring = convert::conversion_allowed() && !convert::gate::is_standalone();
    PIN_WRITES.get().unwrap_or(WRITES) && authoring
}

/// Write `bytes` to `file`, saying whether it was written (a warning if not).
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
/// build, and kept for a model outside a project too, in the system temp folder
/// ([`cooked_path`]): what a test of either kind of build needs. A standalone
/// game or Play still writes none. A test hook (`crates/CLAUDE.md`, *Test
/// hooks*): public only for the `dev` crate's tests, never called by the engine.
#[doc(hidden)]
pub fn with_writes<R>(on: bool, f: impl FnOnce() -> R) -> R {
    /// Restores the previous pin even if `f` panics.
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
