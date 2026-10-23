//! The file names and URIs of the textures a conversion writes or references,
//! shared by every converter that writes its own (`fbx::texture`, `blend::pack`).
// A converter's (#1172): see `fbx`'s root for why.
#![deny(
    clippy::let_underscore_must_use,
    clippy::unused_result_ok,
    clippy::print_stdout,
    clippy::print_stderr
)]

use crate::asset::gltf::uri;
use crate::asset::IMAGE_EXTENSIONS;
use crate::core::paths;
use std::path::{Component, Path};

/// The sanitised stem and lowercase extension of the file `raw` names, holding
/// `bytes`: only ASCII letters, digits, `-`, `_` and inner `.`, so no separator,
/// `..` or URI escape survives. A name without an image extension whose bytes are
/// a PNG or a JPEG gets that extension (`Material #25` → `Material__25.png`).
pub(super) fn file_name(raw: &str, bytes: &[u8]) -> (String, String) {
    // FBX paths are often Windows ones: split on both separators.
    let base = raw.rsplit(['/', '\\']).next().unwrap_or(raw);
    let (stem, ext) = base.rsplit_once('.').unwrap_or((base, ""));
    let ext = clean(ext).to_ascii_lowercase();
    let (stem, ext) = match sniff(bytes) {
        Some(sniffed) if !IMAGE_EXTENSIONS.contains(&ext.as_str()) => {
            (clean(base), sniffed.to_string())
        }
        _ => (clean(stem), ext),
    };
    let stem = if stem.is_empty() {
        "texture".to_string()
    } else {
        stem
    };
    (stem, ext)
}

/// `s` with every character but ASCII letters, digits, `-`, `_` and `.` made `_`,
/// and no leading or trailing `.`.
fn clean(s: &str) -> String {
    let s: String = s
        .chars()
        .map(|c| match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '.' => c,
            _ => '_',
        })
        .collect();
    s.trim_matches('.').to_string()
}

/// The extension `bytes`' magic number says they are, for the formats a name
/// alone may not tell: PNG and JPEG.
fn sniff(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("png")
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("jpg")
    } else {
        None
    }
}

/// `<stem>.<ext>`, or `<stem>.1.<ext>`, … when a name already `taken` matches it
/// ignoring case (a macOS disk would make the two one file).
pub(super) fn unique(stem: &str, ext: &str, taken: &[String]) -> String {
    let free = |name: &str| !taken.iter().any(|t| t.eq_ignore_ascii_case(name));
    let base = format!("{stem}.{ext}");
    if free(&base) {
        return base;
    }
    (1..)
        .map(|n| format!("{stem}.{n}.{ext}"))
        .find(|name| free(name))
        .unwrap_or(base)
}

/// `target` as a `/`-separated, percent-encoded URI from directory `from`, both
/// resolved first.
pub(super) fn relative_uri(from: &Path, target: &Path) -> Option<String> {
    let from = paths::canonicalize(from).ok()?;
    let target = paths::canonicalize(target).ok()?;
    let from: Vec<Component> = from.components().collect();
    let target: Vec<Component> = target.components().collect();
    let shared = from.iter().zip(&target).take_while(|(a, b)| a == b).count();
    let up = std::iter::repeat_n("..".to_string(), from.len() - shared);
    let down = target[shared..]
        .iter()
        .map(|c| c.as_os_str().to_string_lossy().into_owned());
    Some(uri::encode_path(
        &up.chain(down).collect::<Vec<_>>().join("/"),
    ))
}
