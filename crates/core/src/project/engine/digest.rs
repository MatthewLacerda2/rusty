//! crates/core/src/project/engine/digest.rs — one hash of the engine's content folder.
//!
//! Every file under `engine/` (shaders, fonts, scripts, models, textures), by its
//! sorted `/`-separated relative path, its length and its bytes, folded into one
//! 64-bit xxh3 hash (`core::hash`'s content hash), printed as 16 hex digits. A
//! symbolic link is followed to what it names. A name starting with `.` (a Finder
//! `.DS_Store`, an editor's swap file) is not content and is skipped.
//!
//! The root package's `build.rs` includes this file with `#[path]` to stamp what a
//! `dev` binary was built with, and `content` hashes the folder on disk with it at
//! run time, so both hash alike. It names only std and `xxhash_rust`, which both
//! `rusty-core` and the root package's build script depend on.

use std::path::Path;

/// The digest of every file under `dir`, or why it could not be taken: the folder
/// or an entry under it could not be read, or it holds no file.
pub fn digest(dir: &Path) -> Result<String, String> {
    let mut files = Vec::new();
    walk(dir, "", &mut files)?;
    if files.is_empty() {
        return Err(format!("{} holds no file", dir.display()));
    }
    files.sort();
    let mut hash = xxhash_rust::xxh3::Xxh3::new();
    for rel in &files {
        let path = dir.join(rel);
        let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        // The path ends at a zero byte and the length fixes where the bytes end,
        // so moving bytes between a name and its file never collides.
        let len = (bytes.len() as u64).to_le_bytes();
        for chunk in [rel.as_bytes(), &[0], &len, &bytes] {
            hash.update(chunk);
        }
    }
    Ok(format!("{:016x}", hash.digest()))
}

/// Every file under `dir/prefix`, as `/`-separated paths relative to `dir`. A link
/// is followed (`fs::metadata`), so a linked folder is walked and a linked file
/// read. An error names the path it failed on.
fn walk(dir: &Path, prefix: &str, out: &mut Vec<String>) -> Result<(), String> {
    let at = dir.join(prefix);
    let why = |path: &Path, e: std::io::Error| format!("{}: {e}", path.display());
    for entry in std::fs::read_dir(&at).map_err(|e| why(&at, e))? {
        let entry = entry.map_err(|e| why(&at, e))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        let rel = if prefix.is_empty() {
            name
        } else {
            format!("{prefix}/{name}")
        };
        let path = entry.path();
        if std::fs::metadata(&path)
            .map_err(|e| why(&path, e))?
            .is_dir()
        {
            walk(dir, &rel, out)?;
        } else {
            out.push(rel);
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "digest_tests.rs"]
mod tests;
