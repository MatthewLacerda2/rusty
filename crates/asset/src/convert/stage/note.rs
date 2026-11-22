//! The write log's half of a conversion: [`swapped_in`] notes the `.glb` and the
//! **source** (what scenes name) so path-keyed caches drop what they read from the
//! old one. Staged paths and the textures folder are never noted: their names carry
//! the tag, so changed textures land at paths no cache has read.

use rusty_core::fs::written::note_written;
use std::path::Path;

pub(in crate::convert) fn write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    // lint: allow(asset-write) a staged file no cache has seen; `swapped_in` notes its final path
    std::fs::write(path, bytes)
}

pub(super) fn swapped_in(source: &Path, out: &Path) {
    note_written(&out.to_string_lossy());
    note_written(&source.to_string_lossy());
}
