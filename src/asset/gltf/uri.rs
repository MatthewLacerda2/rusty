//! Percent-encoding for the relative URIs a glTF names its files by (#1082).
//!
//! The FBX converter writes a texture's path as a URI ([`encode_path`]) and the
//! importer turns it back into the path ([`decode`]), so a file name holding a `:`,
//! a space or a literal `%` survives the round trip: unencoded, `Soldier: v2.png`
//! reads as a URI scheme and `a%20b.png` as `a b.png`.

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
    use super::{decode, encode_path};

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
    }
}
