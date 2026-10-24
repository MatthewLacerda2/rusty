//! Re-converting only what a change touched (#1327): the editor's file watch notes
//! the paths changed outside the engine, and only the sources among them, or whose
//! last conversion read one of them (an FBX's texture, `depends`), convert again.
//!
//! Play's refresh ([`refresh_sources`](super::refresh_sources)) checks every source;
//! the watch must not, since a source whose conversion fails (a `.blend` that times
//! out in Blender) would run again on every frame that noted anything. Here a failed
//! source is left alone until it, or a file it read, changes again.

use super::{cache_location, cached_gltf, conversion_allowed, convertible_sources, record};
use crate::core::fs::written::Stale;
use std::path::{Path, PathBuf};

/// Convert every source under `root` that `changed` names, or whose cached
/// conversion read a file `changed` names. Returns how many it converted; a failure
/// is warned of, as an import's is. A no-op while conversion is off.
pub fn refresh_changed(root: &Path, changed: Vec<String>) -> usize {
    if !conversion_allowed() {
        return 0;
    }
    let touched = touched(root, &Stale::new(changed));
    let mut converted = 0;
    for source in touched {
        match cached_gltf(&source) {
            Ok(_) => converted += 1,
            Err(e) => log::warn!("[Asset] {}: {e}", source.display()),
        }
    }
    converted
}

/// The sources under `root` a note in `changed` touches, sorted.
fn touched(root: &Path, changed: &Stale) -> Vec<PathBuf> {
    if changed.is_empty() {
        return Vec::new();
    }
    let names = |path: &Path| changed.matches(&path.to_string_lossy());
    let read_one = |source: &Path| {
        let stored = cache_location(source)
            .ok()
            .and_then(|out| record::stored(&out));
        stored.is_some_and(|s| s.depends_on.iter().any(|d| names(d.path())))
    };
    convertible_sources(root)
        .into_iter()
        .filter(|source| names(source) || read_one(source))
        .collect()
}

#[cfg(test)]
#[path = "tests/changed.rs"]
mod tests;
