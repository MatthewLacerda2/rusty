//! Swap order: textures folder in, old stamp removed, `.glb` renamed over the old one,
//! new stamp written (earlier it would bless the old `.glb`), then `clear`.

mod clear;
pub(in crate::convert) mod hook;
mod note;

pub use clear::{textures_dirs, textures_owner};
pub(in crate::convert) use note::write;

use super::{record, stamp_path, write_error, Converted, Converter, ImportError};
use hook::{at, Step};
use rusty_core::fs;
use std::path::{Path, PathBuf};

/// Where one conversion writes: plain files, in a fresh folder of its own.
pub struct Staging {
    /// Where the glTF goes.
    pub glb: PathBuf,
    pub textures: PathBuf,
    pub out: PathBuf,
}

const STAGING: &str = "staging";
const TEXTURES: &str = "textures";

pub(super) fn convert(
    converter: &Converter,
    source: &Path,
    out: &Path,
    (stamp, tag): (&str, &str),
) -> Result<Converted, ImportError> {
    let (dir, name) = clear::split(out);
    let staging = fs::unique_sibling(&dir.join(format!(".{name}")), STAGING);
    std::fs::create_dir(&staging).map_err(|e| write_error(&staging, &e))?;
    let to = Staging {
        glb: staging.join(&name),
        textures: staging.join(textures_name(&name, tag)),
        out: out.to_path_buf(),
    };
    at(Step::Staged);
    let result = (converter.convert)(source, &to)
        .and_then(|converted| swap(&to, &dir, &record(stamp, tag, &converted)).map(|()| converted));
    let _ = std::fs::remove_dir_all(&staging);
    let converted = result?;
    at(Step::Stamped);
    note::swapped_in(source, out);
    clear::clear(&dir, &name);
    crate::texture_import::queue_extracted(out);
    Ok(converted)
}

fn textures_name(name: &str, tag: &str) -> String {
    format!("{name}.{tag}.{TEXTURES}")
}

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
    // lint: allow(asset-write) the conversion stamp, read by the next freshness check; `note` notes the `.glb` and source after the swap
    fs::write_atomic(&stamp_path, record.as_bytes()).map_err(|e| write_error(&stamp_path, &e))
}

/// A `to` that is already a folder holds the same tag's output, so a file name there
/// holds the same bytes: files are replaced one by one and nothing a reader relies on
/// changes.
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

fn remove_file(path: &Path) -> std::io::Result<()> {
    match std::fs::remove_file(path) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}
