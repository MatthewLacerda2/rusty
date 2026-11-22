// A converter's: see `fbx`'s root for why.
#![deny(
    clippy::let_underscore_must_use,
    clippy::unused_result_ok,
    clippy::print_stdout,
    clippy::print_stderr
)]

use super::stage::{
    self,
    hook::{self, Step},
};
use crate::gltf::uri;
use crate::{ImportError, IMAGE_EXTENSIONS};
use std::path::{Component, Path};

/// Only ASCII letters, digits, `-`, `_` and inner `.` survive. The bytes' magic number
/// wins over the name: the renderer picks its decoder by extension, and a `wood.jpg`
/// packed into a `.blend` may hold PNG. `Material #25` → `Material__25.png`.
pub(super) fn file_name(raw: &str, bytes: &[u8]) -> (String, String) {
    // FBX paths are often Windows ones: split on both separators.
    let base = raw.rsplit(['/', '\\']).next().unwrap_or(raw);
    let (stem, ext) = base.rsplit_once('.').unwrap_or((base, ""));
    let ext = clean(ext).to_ascii_lowercase();
    let named = IMAGE_EXTENSIONS.contains(&ext.as_str()) || ext == "exr";
    let (stem, ext) = match sniff(bytes) {
        Some("jpg") if ext == "jpeg" => (clean(stem), ext),
        Some(sniffed) if named => (clean(stem), sniffed.to_string()),
        Some(sniffed) => (clean(base), sniffed.to_string()),
        None => (clean(stem), ext),
    };
    let stem = if stem.is_empty() {
        "texture".to_string()
    } else {
        stem
    };
    (stem, ext)
}

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

/// TGA has no magic number, so its name is trusted.
pub(super) fn sniff(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("png")
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("jpg")
    } else if bytes.starts_with(&[0x76, 0x2F, 0x31, 0x01]) {
        Some("exr")
    } else {
        None
    }
}

/// Never creates the staging folder above `dir`: a concurrent conversion that cleared
/// it must make this one fail, not resurrect half of it.
pub(super) fn write(dir: &Path, name: &str, bytes: &[u8]) -> Result<(), ImportError> {
    match std::fs::create_dir(dir) {
        Err(e) if e.kind() != std::io::ErrorKind::AlreadyExists => {
            return Err(ImportError::Parse(format!(
                "writing {}: {e}",
                dir.display()
            )))
        }
        _ => {}
    }
    let path = dir.join(name);
    hook::io(Step::ExtractTexture)
        .and_then(|()| stage::write(&path, bytes))
        .map_err(|e| ImportError::Parse(format!("writing {}: {e}", path.display())))
}

/// Case-insensitive against `taken`: a macOS disk would make the two one file.
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

pub(super) fn relative_uri(from: &Path, target: &Path) -> Option<String> {
    let from = std::fs::canonicalize(from).ok()?;
    let target = std::fs::canonicalize(target).ok()?;
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
