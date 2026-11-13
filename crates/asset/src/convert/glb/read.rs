//! What a reference to an extracted texture needs from a written `.glb` (#1130):
//! the URI of each named image, read from the JSON chunk alone, so resolving a
//! texture never reads the geometry behind it.

use super::write::{CHUNK_JSON, GLB_MAGIC};
use serde_json::Value;
use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;

/// The URI of each named image in the `.glb` at `path`, by name, as written
/// (percent-encoded, relative to the file's folder); the first image of a name
/// keeps it. `None` when the file is not a `.glb`.
pub fn image_uris(path: &Path) -> Option<BTreeMap<String, String>> {
    let mut uris = BTreeMap::new();
    for (name, uri) in images(path)? {
        if let Some(name) = name {
            uris.entry(name).or_insert(uri);
        }
    }
    Some(uris)
}

/// The URI of every image in the `.glb` at `path` that names a file, named or
/// not, in order, as written. `None` when the file is not a `.glb`.
pub fn image_files(path: &Path) -> Option<Vec<String>> {
    Some(images(path)?.into_iter().map(|(_, uri)| uri).collect())
}

/// Each image with a URI, as (name, URI).
fn images(path: &Path) -> Option<Vec<(Option<String>, String)>> {
    let doc: Value = serde_json::from_slice(&json_chunk(path)?).ok()?;
    let images = doc.get("images").and_then(Value::as_array);
    let named = images.into_iter().flatten().filter_map(|image| {
        let name = image
            .get("name")
            .and_then(Value::as_str)
            .map(str::to_string);
        let uri = image.get("uri").and_then(Value::as_str)?;
        Some((name, uri.to_string()))
    });
    Some(named.collect())
}

/// The JSON chunk of the `.glb` at `path`: the 12-byte header, then the first
/// chunk, which glTF 2.0 §4.4 requires to be the JSON one.
fn json_chunk(path: &Path) -> Option<Vec<u8>> {
    let mut file = std::fs::File::open(path).ok()?;
    let mut head = [0u8; 20];
    file.read_exact(&mut head).ok()?;
    let word = |at: usize| u32::from_le_bytes([head[at], head[at + 1], head[at + 2], head[at + 3]]);
    if word(0) != GLB_MAGIC || word(16) != CHUNK_JSON {
        return None;
    }
    // Bounded by the file before allocating: a corrupt length word must not ask
    // for gigabytes.
    let len = u64::from(word(12));
    if len > file.metadata().ok()?.len().saturating_sub(20) {
        return None;
    }
    let mut json = vec![0; usize::try_from(len).ok()?];
    file.read_exact(&mut json).ok()?;
    Some(json)
}
