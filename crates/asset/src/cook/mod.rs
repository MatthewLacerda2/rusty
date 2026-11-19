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
//! an export), elsewhere the system temp folder. It is a glTF ([`file`]) holding
//! each sub-mesh's tangents, lightmap UVs, its cluster-ordered indices and its
//! cluster table ([`sub`]), stamped by what it was made from ([`stamp`]). An
//! import whose stamp matches parses the model without generating tangents and
//! lays the cooked values onto it: it computes none of the three, and the result
//! equals a fresh cook bit for bit, so a baked lightmap fits a cooked load as it
//! fits a fresh one. A missing, stale or malformed file is a miss: the model is
//! cooked as before ([`cooks`] counts those).
//!
//! **Written** on a miss by an authoring build ([`WRITES`]: the editor or `dev`)
//! while conversion is allowed (edit mode, never Play or a standalone game); a
//! shipped player reads cooked files, never writes one, and cooks in memory on a
//! miss.
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

thread_local!(static COOKS: Cell<usize> = const { Cell::new(0) });

/// How many models this thread has cooked (run the steps for, a cooked file
/// found current not counted): what tells a warm import from a cold one.
pub fn cooks() -> usize {
    COOKS.get()
}

/// Where the cooked file of the model source `source` lives.
pub fn cooked_path(source: &Path) -> Option<PathBuf> {
    convert::cache_location_as(source, SUFFIX).ok()
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
    let at = cooked_path(path).zip(stamp::stamp(read, settings));
    if let Some(cooked) = at
        .as_ref()
        .and_then(|(file, stamp)| read_cooked(file, *stamp))
    {
        let mut asset = parse(Missing::Placeholder)?;
        scale(&mut asset);
        match lay(&mut asset, cooked) {
            Ok(()) => return Ok(asset),
            Err(why) => log::warn!("[Import] {}: {why}; cooking it again", path.display()),
        }
    }
    let mut asset = parse(Missing::Generate)?;
    scale(&mut asset);
    let sources = derive(&mut asset, settings);
    COOKS.set(COOKS.get() + 1);
    if let Some((file, stamp)) = at.filter(|_| writes()) {
        let subs: Vec<Cooked> = (asset.sub_meshes.iter().zip(sources))
            .map(|(s, source)| Cooked::of(s, source))
            .collect();
        write(&file, &file::encode(stamp, &subs));
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
    par::map(subs, |sub| {
        let source = match settings.lightmap_uvs.generate {
            true => lightmap_uv::apply(sub, &settings.lightmap_uvs),
            false => None,
        };
        clusters::apply(sub);
        source
    })
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
    asset
        .sub_meshes
        .iter_mut()
        .zip(cooked)
        .try_for_each(|(s, c)| c.apply(s))
}

/// Whether a cook on this thread is written to disk ([module docs](self)).
fn writes() -> bool {
    pinned_writes().unwrap_or(WRITES) && convert::conversion_allowed()
}

fn write(file: &Path, bytes: &[u8]) {
    let written = file
        .parent()
        .map_or(Ok(()), std::fs::create_dir_all)
        // lint: allow(asset-write) a cache of values the import just computed; read only when a model is parsed
        .and_then(|()| rusty_core::fs::write_atomic(file, bytes));
    if let Err(e) = written {
        log::warn!("[Import] cannot write {}: {e}", file.display());
    }
}

#[cfg(test)]
thread_local!(static PIN_WRITES: Cell<Option<bool>> = const { Cell::new(None) });

/// Run `f` with cooked files written (`true`) or not on this thread, whatever the
/// build: what a test of either kind of build needs.
#[cfg(test)]
pub(crate) fn with_writes<R>(on: bool, f: impl FnOnce() -> R) -> R {
    let was = PIN_WRITES.replace(Some(on));
    let out = f();
    PIN_WRITES.set(was);
    out
}

fn pinned_writes() -> Option<bool> {
    #[cfg(test)]
    return PIN_WRITES.get();
    #[cfg(not(test))]
    None
}

#[cfg(test)]
mod file_tests;
#[cfg(test)]
pub(crate) mod fixture;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod version;
