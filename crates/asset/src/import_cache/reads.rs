//! The files besides the parsed one whose writes make a cached import stale: an
//! `.obj`'s `mtllib` files and a glTF's external buffers. Not textures, which the
//! importer only names. `tobj` takes the rest of an `mtllib` line as one file name.

use std::io::Read;
use std::path::{Path, PathBuf};

use std::time::Duration;

use crate::gltf::uri;
use rusty_core::paths;

/// `None`: it could not be read.
pub(super) type Stamp = Option<(u64, Option<Duration>)>;

pub(super) type Files = Vec<(String, Stamp)>;

/// For each `borrowed` model: its parse's files, its converted source, and its
/// sidecar (whose humanoid map shapes the clips).
pub(super) fn files_of(read: &Path, borrowed: &[String]) -> Files {
    let own = std::iter::once(read.to_path_buf()).chain(reads(read));
    let sources = borrowed.iter().flat_map(|from| {
        let from = Path::new(from);
        let read = crate::read_target(from).unwrap_or_else(|_| from.to_path_buf());
        let meta = crate::sidecar::meta_path(from);
        let own = (read != from).then(|| from.to_path_buf());
        std::iter::once(read.clone())
            .chain(reads(&read))
            .chain(own)
            .chain([meta])
    });
    own.chain(sources)
        .map(|p| (p.to_string_lossy().into_owned(), stamp(&p)))
        .collect()
}

pub(crate) fn parsed(read: &Path) -> Vec<PathBuf> {
    std::iter::once(read.to_path_buf())
        .chain(reads(read))
        .collect()
}

pub(super) fn stamp(path: &Path) -> Stamp {
    let meta = std::fs::metadata(path).ok()?;
    let modified = meta.modified().ok();
    let since = modified.and_then(|m| m.duration_since(std::time::UNIX_EPOCH).ok());
    Some((meta.len(), since))
}

fn reads(read: &Path) -> Vec<PathBuf> {
    let base = read.parent().unwrap_or_else(|| Path::new(""));
    match paths::extension(read).as_str() {
        "obj" => std::fs::read(read)
            .map(|b| mtllibs(&b, base))
            .unwrap_or_default(),
        "gltf" => std::fs::read(read)
            .map(|j| buffers(&j, base))
            .unwrap_or_default(),
        "glb" => glb_json(read)
            .map(|j| buffers(&j, base))
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

fn mtllibs(obj: &[u8], base: &Path) -> Vec<PathBuf> {
    let text = String::from_utf8_lossy(obj);
    let named = text.lines().filter_map(|line| {
        let line = line.trim_start();
        let rest = line.strip_prefix("mtllib")?;
        let name = rest.starts_with(char::is_whitespace).then(|| rest.trim())?;
        (!name.is_empty()).then(|| base.join(name))
    });
    named.collect()
}

fn buffers(json: &[u8], base: &Path) -> Vec<PathBuf> {
    let Ok(root) = gltf::json::deserialize::from_slice::<gltf::json::Root>(json) else {
        return Vec::new();
    };
    let uris = root.buffers.iter().filter_map(|b| b.uri.as_deref());
    let files = uris.filter(|u| !u.starts_with("data:"));
    files.filter_map(|u| uri::file(base, u)).collect()
}

/// A `.glb`'s JSON chunk: 12-byte header, then the first chunk's length and type.
pub(crate) fn glb_json(path: &Path) -> Option<Vec<u8>> {
    let mut file = std::fs::File::open(path).ok()?;
    let mut head = [0u8; 20];
    file.read_exact(&mut head).ok()?;
    let word = |at: usize| u32::from_le_bytes([head[at], head[at + 1], head[at + 2], head[at + 3]]);
    // `glTF` magic, then a `JSON` first chunk.
    if word(0) != 0x4654_6C67 || word(16) != 0x4E4F_534A {
        return None;
    }
    let len = usize::try_from(word(12)).ok()?;
    let total = usize::try_from(word(8)).ok()?;
    if len > total {
        return None;
    }
    let mut json = vec![0u8; len];
    file.read_exact(&mut json).ok()?;
    Some(json)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_obj_reads_its_mtllibs() {
        let obj = b"# x\nmtllib a b.mtl\n  mtllib  c.mtl \nmtllibx d.mtl\nmtllib\nv 0 0 0\n";
        let got = mtllibs(obj, Path::new("dir"));
        let dir = Path::new("dir");
        let want = [dir.join("a b.mtl"), dir.join("c.mtl")];
        assert_eq!(got, want);
    }

    #[test]
    fn a_gltf_reads_its_external_buffers_only() {
        let json = br#"{"asset":{"version":"2.0"},"buffers":[
            {"byteLength":1,"uri":"m%20v.bin"},
            {"byteLength":1,"uri":"data:application/octet-stream;base64,AA=="},
            {"byteLength":1}]}"#;
        let want = [Path::new("d").join("m v.bin")];
        assert_eq!(buffers(json, Path::new("d")), want);
        assert!(buffers(b"not json", Path::new("d")).is_empty());
    }

    #[test]
    fn a_truncated_glb_reads_nothing() {
        let dir = rusty_test_support::temp::dir().join("rusty_import_cache_reads");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("cut.glb");
        std::fs::write(&path, b"glTF\x02\0\0\0").unwrap();
        assert!(reads(&path).is_empty());
        assert!(reads(&dir.join("absent.obj")).is_empty());
    }
}
