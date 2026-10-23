//! What a reference to an extracted texture needs from a written `.glb` (#1130):
//! the URI of one named image, read from the JSON chunk alone, so resolving a
//! texture never reads the geometry behind it.

use super::write::{CHUNK_JSON, GLB_MAGIC};
use serde_json::Value;
use std::io::Read;
use std::path::Path;

/// The URI of the image named `name` in the `.glb` at `path`, as written
/// (percent-encoded, relative to the file's folder); `None` when the file is not
/// a `.glb` or names no such image.
pub fn image_uri(path: &Path, name: &str) -> Option<String> {
    let doc: Value = serde_json::from_slice(&json_chunk(path)?).ok()?;
    let image = doc
        .get("images")?
        .as_array()?
        .iter()
        .find(|image| image.get("name").and_then(Value::as_str) == Some(name))?;
    image.get("uri")?.as_str().map(str::to_string)
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
    let mut json = vec![0; usize::try_from(word(12)).ok()?];
    file.read_exact(&mut json).ok()?;
    Some(json)
}
