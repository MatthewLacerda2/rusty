//! The root `build.rs` includes this file with `#[path]`, so it names only std and
//! `xxhash_rust`, that script's one build-dependency.
use std::path::Path;

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
