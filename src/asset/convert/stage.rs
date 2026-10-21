//! src/asset/convert/stage.rs — a conversion written into a staging folder and
//! swapped in whole (#1129).
//!
//! A reader in another process must see the old cache whole or the new one whole:
//! never a new `.glb` pointing at textures not there yet, nor an old one whose
//! textures were deleted. On Linux and macOS `rename` replaces a file atomically,
//! but moves a folder only onto a missing or empty one, so the swap runs:
//!
//! 1. The converter writes plain files into `.<cache>.<pid>.<n>.staging/` beside
//!    the cache: the same filesystem, so every later move is a rename.
//! 2. Its textures folder is renamed in. A folder cannot replace a full one, so each
//!    conversion's folder has its own name, `<cache>.<digest>.textures`, and the
//!    old `.glb`'s folder stays untouched beside it.
//! 3. The `.glb` is renamed over the old one: the one step a reader can observe.
//! 4. The stamp is written. After the `.glb`, never before: a new stamp beside the
//!    old `.glb` would call it up to date.
//! 5. Older textures folders, and staging folders a crashed conversion left, are
//!    removed.
//!
//! A failing converter stops before step 2: its staging folder is removed and the
//! old cache is as it was.

use super::{stamp_path, write_error, Converter, ImportError};
use crate::core::{fs, hash};
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
/// result in, stamped `stamp`.
pub(super) fn convert(
    converter: &Converter,
    source: &Path,
    out: &Path,
    stamp: &str,
) -> Result<(), ImportError> {
    let (dir, name) = split(out);
    let tag = hash::fnv1a64(stamp.as_bytes());
    let staging = fs::unique_sibling(&dir.join(format!(".{name}")), STAGING);
    std::fs::create_dir(&staging).map_err(|e| write_error(&staging, &e))?;
    let to = Staging {
        glb: staging.join(&name),
        textures: staging.join(format!("{name}.{tag:016x}.{TEXTURES}")),
        out: out.to_path_buf(),
    };
    let result = (converter.convert)(source, &to).and_then(|()| swap(&to, &dir, stamp));
    let _ = std::fs::remove_dir_all(&staging);
    result?;
    clear(&dir, &name, &to.textures);
    Ok(())
}

/// Steps 2 to 4: move the staged set into `dir`, the `.glb` last, then stamp it.
fn swap(to: &Staging, dir: &Path, stamp: &str) -> Result<(), ImportError> {
    if let (true, Some(name)) = (to.textures.is_dir(), to.textures.file_name()) {
        move_in(&to.textures, &dir.join(name))?;
    }
    std::fs::rename(&to.glb, &to.out).map_err(|e| write_error(&to.out, &e))?;
    let stamp_path = stamp_path(&to.out);
    fs::write_atomic(&stamp_path, stamp.as_bytes()).map_err(|e| write_error(&stamp_path, &e))
}

/// Move the folder `from` to `to`: whole when `to` is missing, else file by file.
/// `to` then holds the same conversion's output (same digest), kept when its stamp
/// was lost, so replacing its files changes nothing a reader relies on.
fn move_in(from: &Path, to: &Path) -> Result<(), ImportError> {
    if std::fs::rename(from, to).is_ok() {
        return Ok(());
    }
    let entries = std::fs::read_dir(from).map_err(|e| write_error(from, &e))?;
    for entry in entries {
        let entry = entry.map_err(|e| write_error(from, &e))?;
        let dest = to.join(entry.file_name());
        std::fs::rename(entry.path(), &dest).map_err(|e| write_error(&dest, &e))?;
    }
    Ok(())
}

/// Step 5: remove what earlier conversions of `name` left in `dir`, keeping the
/// textures folder staged at `keep`. A staging folder still in use belongs to a
/// concurrent conversion of the same source in another process: its `refresh`
/// then finds this conversion in place, or fails that once.
fn clear(dir: &Path, name: &str, keep: &Path) {
    let keep = keep.file_name().map(|k| dir.join(k));
    for old in textures_dirs(&dir.join(name)) {
        if Some(&old) != keep.as_ref() {
            let _ = std::fs::remove_dir_all(old);
        }
    }
    let staging_of = format!(".{name}");
    for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        if fs::is_unique_sibling(&entry.file_name().to_string_lossy(), &staging_of, STAGING) {
            let _ = std::fs::remove_dir_all(entry.path());
        }
    }
}

/// The textures folders beside the cached glTF at `out`, current or left by an
/// earlier conversion: `<out>.<digest>.textures/`, and the undigested
/// `<out>.textures/` of converters before #1129.
pub fn textures_dirs(out: &Path) -> Vec<PathBuf> {
    let (dir, name) = split(out);
    std::fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| is_textures_of(&e.file_name().to_string_lossy(), &name))
        .map(|e| e.path())
        .collect()
}

fn is_textures_of(entry: &str, name: &str) -> bool {
    let Some(middle) = entry
        .strip_prefix(name)
        .and_then(|rest| rest.strip_suffix(TEXTURES))
    else {
        return false;
    };
    let tag = middle.strip_prefix('.').and_then(|m| m.strip_suffix('.'));
    middle == "." || tag.is_some_and(|t| t.len() == 16 && t.bytes().all(|b| b.is_ascii_hexdigit()))
}

/// `out`'s folder (`.` for a bare file name) and file name.
fn split(out: &Path) -> (PathBuf, String) {
    let dir = match out.parent() {
        Some(dir) if !dir.as_os_str().is_empty() => dir.to_path_buf(),
        _ => PathBuf::from("."),
    };
    let name = out
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    (dir, name)
}
