use std::collections::BTreeSet;
use std::path::Path;

use rusty_asset::convert::{glb, IMPORT_CACHE_DIR};
use rusty_asset::gltf::uri;
use rusty_core::paths;

pub(super) fn missing(out: &Path, glbs: &[String], written: &[String]) -> Vec<String> {
    let written: BTreeSet<&str> = written.iter().map(String::as_str).collect();
    let mut missing = Vec::new();
    for rel in glbs {
        let rel = Path::new(rel);
        let base = rel.parent().unwrap_or(Path::new(""));
        let uris = glb::image_files(&out.join(rel)).unwrap_or_default();
        for uri in uris.iter().filter(|u| !u.starts_with("data:")) {
            let Some(texture) = uri::file(base, uri) else {
                continue;
            };
            let texture = paths::to_slash(&texture);
            if !written.contains(texture.as_str()) {
                missing.push(format!("{}: {texture}", source_of(rel)));
            }
        }
    }
    missing
}

fn source_of(rel: &Path) -> String {
    let rel = paths::to_slash(rel);
    let source = rel.strip_prefix(IMPORT_CACHE_DIR).unwrap_or(&rel);
    let source = source.strip_prefix('/').unwrap_or(source);
    source.strip_suffix(".glb").unwrap_or(source).to_string()
}
