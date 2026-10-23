//! src/asset/convert/stage/ — a conversion written into a staging folder and
//! swapped in whole (#1129).
//!
//! A reader in another process must find the set whole **at the moment it opens
//! the `.glb`**: never a new `.glb` pointing at textures not there yet, nor an old
//! one whose textures were deleted. On Linux and macOS `rename` replaces a file
//! atomically, but moves a folder only onto a missing or empty one, so the swap
//! runs:
//!
//! 1. The converter writes plain files into `.<cache>.<pid>.<n>.staging/` beside
//!    the cache: the same filesystem, so every later move is a rename.
//! 2. Its textures folder is renamed in. A folder cannot replace a full one, so each
//!    conversion's folder has its own name, `<cache>.<digest>.textures`, and the
//!    old `.glb`'s folder stays untouched beside it.
//! 3. The old stamp is removed, then the `.glb` is renamed over the old one: the
//!    one step a reader can observe. A crash between the two leaves an unstamped
//!    cache, which re-converts, never a stale `.glb` called current.
//! 4. The new stamp is written, with what the conversion skipped under it. After
//!    the `.glb`, never before: a new stamp beside the old `.glb` would call it up
//!    to date.
//! 5. Textures folders the stamp **on disk** does not name, staging folders and
//!    stamp temp files a crashed conversion left, are removed (`clear`).
//!
//! Between steps 4 and 5 the swapped-in `.glb` and the source are noted in the
//! write log, `core::fs::written` (`note`, #1219): a converter writes staged
//! files through [`write`] and never notes them itself.
//!
//! A failing converter stops before step 2: its staging folder is removed and the
//! old cache is as it was.
//!
//! **Two conversions at once.** `refresh` holds the cache's lock (`lock`), so a
//! second conversion of it waits for the first. Without one (a filesystem with
//! no locks), the set a reader opens stays whole as long as the second conversion
//! does not land between another's steps 3 and 4: `clear` keeps the folder the
//! stamp on disk names, and a converter whose staging folder was cleared fails
//! that one import.

mod clear;
pub(in crate::asset::convert) mod hook;
mod note;

pub use clear::textures_dirs;
pub(in crate::asset::convert) use note::write;

use super::{record, stamp_path, write_error, Converted, Converter, ImportError};
use crate::core::{fs, hash};
use hook::{at, Step};
use std::path::{Path, PathBuf};

/// Where one conversion writes: plain files, in a fresh folder of its own.
pub struct Staging {
    /// Where the glTF goes.
    pub glb: PathBuf,
    /// Where the files the glTF references go, created by the converter when it
    /// writes one. The glTF names them `<this folder's name>/<file>`, which
    /// resolves the same once both are swapped into place.
    pub textures: PathBuf,
    /// Where the glTF will live: a file the glTF references where it lies (a
    /// texture on disk) is named relative to this one's folder.
    pub out: PathBuf,
}

const STAGING: &str = "staging";
const TEXTURES: &str = "textures";

/// Run `converter` on `source` into a staging folder beside `out`, and swap the
/// result in, stamped `stamp`: what the converter reported, once it is in place.
pub(super) fn convert(
    converter: &Converter,
    source: &Path,
    out: &Path,
    stamp: &str,
) -> Result<Converted, ImportError> {
    let (dir, name) = clear::split(out);
    let staging = fs::unique_sibling(&dir.join(format!(".{name}")), STAGING);
    std::fs::create_dir(&staging).map_err(|e| write_error(&staging, &e))?;
    let to = Staging {
        glb: staging.join(&name),
        textures: staging.join(textures_name(&name, stamp)),
        out: out.to_path_buf(),
    };
    at(Step::Staged);
    let result = (converter.convert)(source, &to)
        .and_then(|converted| swap(&to, &dir, &record(stamp, &converted)).map(|()| converted));
    let _ = std::fs::remove_dir_all(&staging);
    let converted = result?;
    at(Step::Stamped);
    note::swapped_in(source, out);
    clear::clear(&dir, &name);
    Ok(converted)
}

/// `<name>.<digest>.textures`: the folder of the conversion stamped `stamp`.
fn textures_name(name: &str, stamp: &str) -> String {
    let tag = hash::fnv1a64(stamp.as_bytes());
    format!("{name}.{tag:016x}.{TEXTURES}")
}

/// Steps 2 to 4: move the staged set into `dir`, the `.glb` last, then write its
/// stamp file, `record`.
fn swap(to: &Staging, dir: &Path, record: &str) -> Result<(), ImportError> {
    let moved = match (to.textures.is_dir(), to.textures.file_name()) {
        (true, Some(name)) => move_in(&to.textures, &dir.join(name))?,
        _ => None,
    };
    at(Step::TexturesIn);
    let stamp_path = stamp_path(&to.out);
    let glb = remove_file(&stamp_path).and_then(|()| {
        // lint: allow(atomic-write) step 3 of the swap: the staged `.glb` replaces the old one in one rename
        std::fs::rename(&to.glb, &to.out)
    });
    if let Err(e) = glb {
        // The old `.glb` stays, unstamped: the folder moved in for this one is no one's.
        if let Some(orphan) = moved {
            let _ = std::fs::remove_dir_all(orphan);
        }
        return Err(write_error(&to.out, &e));
    }
    at(Step::GlbIn);
    fs::write_atomic(&stamp_path, record.as_bytes()).map_err(|e| write_error(&stamp_path, &e))
}

/// Move the folder `from` to `to`, returning `to` when it moved whole. When `to`
/// is already a folder, it holds the same conversion's output (same digest), kept
/// when its stamp was lost or a file the conversion looked at changed (#1189): a
/// file name there holds the same bytes whatever was found on disk (`fbx::texture`
/// reserves every embedded texture's name), so its files are replaced one by one
/// and nothing a reader relies on changes. Any other failure is reported as it is.
fn move_in(from: &Path, to: &Path) -> Result<Option<PathBuf>, ImportError> {
    if !to.is_dir() {
        // lint: allow(atomic-write) step 2 of the swap: a staged folder moves in whole
        std::fs::rename(from, to).map_err(|e| write_error(to, &e))?;
        return Ok(Some(to.to_path_buf()));
    }
    let entries = std::fs::read_dir(from).map_err(|e| write_error(from, &e))?;
    for entry in entries {
        let entry = entry.map_err(|e| write_error(from, &e))?;
        let dest = to.join(entry.file_name());
        // lint: allow(atomic-write) step 2 of the swap: a staged file replaces its identical twin
        std::fs::rename(entry.path(), &dest).map_err(|e| write_error(&dest, &e))?;
    }
    Ok(None)
}

/// Remove the file at `path`; one already gone is no failure.
fn remove_file(path: &Path) -> std::io::Result<()> {
    match std::fs::remove_file(path) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}
