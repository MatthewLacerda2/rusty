//! Percent-encoding for the relative URIs a glTF names its files by (#1082).
//!
//! The FBX converter writes a texture's path as a URI ([`encode_path`]) and the
//! importer turns it back into the path ([`decode`]), so a file name holding a `:`,
//! a space or a literal `%` survives the round trip: unencoded, `Soldier: v2.png`
//! reads as a URI scheme and `a%20b.png` as `a b.png`.

use crate::core::paths;
use std::path::{Path, PathBuf};

/// The file a glTF's relative `uri` names, anchored at `base_dir` (the glTF's
/// folder): decoded, joined, and folded lexically (`paths::fold`), so a URI that
/// climbs out of the import cache names the file where it lies, never a path
/// through the cache (#1130). A URI that doesn't decode is warned about and
/// names nothing. The one join the importer and `convert::texture_ref` share.
pub fn file(base_dir: &Path, uri: &str) -> Option<PathBuf> {
    let Some(decoded) = decode(uri) else {
        log::warn!("[Asset] glTF texture URI `{uri}` is not valid percent-encoding");
        return None;
    };
    Some(paths::fold(&base_dir.join(decoded)))
}

/// `path`'s `/`-separated segments with every byte outside RFC 3986's unreserved
/// set (`A-Z a-z 0-9 - . _ ~`) percent-encoded, the `/`s kept.
pub fn encode_path(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    for byte in path.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' => {
                out.push(char::from(byte));
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// `uri` with every `%XX` escape decoded, or `None` when an escape is cut short,
/// isn't hex, or the bytes aren't UTF-8.
pub fn decode(uri: &str) -> Option<String> {
    let bytes = uri.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = std::str::from_utf8(bytes.get(i + 1..i + 3)?).ok()?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

#[cfg(test)]
mod tests {
    use super::{decode, encode_path, file};
    use std::path::Path;

    #[test]
    fn a_path_round_trips_whatever_its_name_holds() {
        for path in [
            "Soldier: v2.glb.textures/a.png",
            "x%C0/y",
            "a%20b/c d",
            "../é#1",
        ] {
            assert_eq!(decode(&encode_path(path)).as_deref(), Some(path), "{path}");
        }
        assert_eq!(encode_path("../a b/c:d.png"), "../a%20b/c%3Ad.png");
    }

    #[test]
    fn a_broken_escape_decodes_to_nothing() {
        assert_eq!(decode("a%2"), None);
        assert_eq!(decode("a%zz"), None);
        assert_eq!(decode("%C0"), None, "not UTF-8");
        assert_eq!(decode("plain/ok.png").as_deref(), Some("plain/ok.png"));
        assert_eq!(file(Path::new("m"), "a%2"), None);
    }

    #[test]
    fn a_uri_climbing_out_of_its_folder_names_the_file_where_it_lies() {
        let base = Path::new("./assets/generated/imported/models");
        let found = file(base, "../../../../models/a%20b.png");
        assert_eq!(found.as_deref(), Some(Path::new("models/a b.png")));
    }
}
