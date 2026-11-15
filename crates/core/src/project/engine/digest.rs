//! crates/core/src/project/engine/digest.rs — one hash of the engine's content folder.
//!
//! Every file under `engine/` (shaders, fonts, scripts, models, textures), by its
//! sorted `/`-separated relative path, its length and its bytes, folded into one
//! 64-bit FNV-1a hash, printed as 16 hex digits. A name starting with `.` (a
//! Finder `.DS_Store`, an editor's swap file) is not content and is skipped.
//!
//! The file is std-only: the root package's `build.rs` includes it with `#[path]`
//! to stamp what a `dev` binary was built with, and `content` hashes the folder on
//! disk with it at run time, so both hash alike.

use std::path::Path;

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// The digest of every file under `dir`; `None` when the folder can't be read or
/// holds no file.
pub fn digest(dir: &Path) -> Option<String> {
    let mut files = Vec::new();
    walk(dir, "", &mut files).ok()?;
    if files.is_empty() {
        return None;
    }
    files.sort();
    let mut hash = FNV_OFFSET;
    for rel in &files {
        let bytes = std::fs::read(dir.join(rel)).ok()?;
        // The path ends at a zero byte and the length fixes where the bytes end,
        // so moving bytes between a name and its file never collides.
        let len = (bytes.len() as u64).to_le_bytes();
        for chunk in [rel.as_bytes(), &[0], &len, &bytes] {
            hash = fold(hash, chunk);
        }
    }
    Some(format!("{hash:016x}"))
}

fn fold(mut hash: u64, bytes: &[u8]) -> u64 {
    for &b in bytes {
        hash = (hash ^ u64::from(b)).wrapping_mul(FNV_PRIME);
    }
    hash
}

/// Every file under `dir/prefix`, as `/`-separated paths relative to `dir`.
fn walk(dir: &Path, prefix: &str, out: &mut Vec<String>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir.join(prefix))? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        let rel = if prefix.is_empty() {
            name
        } else {
            format!("{prefix}/{name}")
        };
        if entry.file_type()?.is_dir() {
            walk(dir, &rel, out)?;
        } else {
            out.push(rel);
        }
    }
    Ok(())
}
