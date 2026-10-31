//! src/asset/import_cache/reads.rs — the other files an import reads (#857).
//!
//! A cached import is current only while every file its parse read still reads
//! the same, as a conversion is only while its dependencies do
//! (`convert::depends`): an `.obj`'s `mtllib` files and a `.gltf`'s external
//! `.bin` buffers. Textures are not among them: the importer never decodes an
//! image (`gltf::import`), it names it, and the renderer reads it.
//!
//! Found the way the parsers find them: `tobj` takes the rest of an `mtllib` line
//! as one file name beside the `.obj`, and a buffer's relative URI resolves
//! beside the glTF (`gltf::uri::file`). A `data:` URI and a `.glb`'s own blob are
//! no file. A `.glb`'s JSON chunk is read alone, never its binary one.

use std::io::Read;
use std::path::{Path, PathBuf};

use std::time::Duration;

use crate::asset::gltf::uri;
use crate::core::paths;

/// A file's size and modified time (`None`: it could not be read).
pub(super) type Stamp = Option<(u64, Option<Duration>)>;

/// The files a parse of `read` reads, `read` first, each with its [`Stamp`].
pub(super) type Files = Vec<(String, Stamp)>;

pub(super) fn files_of(read: &Path) -> Files {
    let all = std::iter::once(read.to_path_buf()).chain(reads(read));
    all.map(|p| (p.to_string_lossy().into_owned(), stamp(&p)))
        .collect()
}

pub(super) fn stamp(path: &Path) -> Stamp {
    let meta = std::fs::metadata(path).ok()?;
    let modified = meta.modified().ok();
    let since = modified.and_then(|m| m.duration_since(std::time::UNIX_EPOCH).ok());
    Some((meta.len(), since))
}

/// The files besides `read` a parse of it reads, found or not.
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

/// The `mtllib` files an `.obj`'s text names, beside it.
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

/// The external buffer files a glTF document's JSON names, beside it.
fn buffers(json: &[u8], base: &Path) -> Vec<PathBuf> {
    let Ok(root) = gltf::json::deserialize::from_slice::<gltf::json::Root>(json) else {
        return Vec::new();
    };
    let uris = root.buffers.iter().filter_map(|b| b.uri.as_deref());
    let files = uris.filter(|u| !u.starts_with("data:"));
    files.filter_map(|u| uri::file(base, u)).collect()
}

/// A `.glb`'s JSON chunk: the 12-byte header, then the first chunk's length and
/// type, then its bytes.
fn glb_json(path: &Path) -> Option<Vec<u8>> {
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
        let want = [Path::new("dir/a b.mtl"), Path::new("dir/c.mtl")];
        assert_eq!(got, want);
    }

    #[test]
    fn a_gltf_reads_its_external_buffers_only() {
        let json = br#"{"asset":{"version":"2.0"},"buffers":[
            {"byteLength":1,"uri":"m%20v.bin"},
            {"byteLength":1,"uri":"data:application/octet-stream;base64,AA=="},
            {"byteLength":1}]}"#;
        assert_eq!(buffers(json, Path::new("d")), [Path::new("d/m v.bin")]);
        assert!(buffers(b"not json", Path::new("d")).is_empty());
    }

    #[test]
    fn a_truncated_glb_reads_nothing() {
        let dir = crate::test_temp::dir().join("rusty_import_cache_reads");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("cut.glb");
        std::fs::write(&path, b"glTF\x02\0\0\0").unwrap();
        assert!(reads(&path).is_empty());
        assert!(reads(&dir.join("absent.obj")).is_empty());
    }
}
