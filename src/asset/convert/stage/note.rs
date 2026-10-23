//! The write log's half of a conversion (#1207, #1219): a converter writes plain
//! files into its staging folder ([`write`]), and once the set is swapped in,
//! [`swapped_in`] notes in `core::fs::written` what a reader may already have
//! read at the same path, so a path-keyed cache (the renderer's particle meshes,
//! the conversion's own `depends` memo) drops what it read from the old one.
//!
//! What is noted is the cached `.glb` and the **source**, which is what scenes
//! and mesh emitters name (`soldier.fbx::Body`), so a consumer keyed by the
//! source matches too. A staged path is never noted: no cache has seen it, and it
//! is gone after the swap. Nor is the textures folder: its name carries the
//! conversion's tag (`record::folder_tag`), so changed textures land at new paths
//! no cache has read, and a re-conversion with the same tag puts back the same
//! bytes (`move_in`).
//! The old folder `clear` removes is named by the old `.glb` only, so an upload
//! still holding one of its paths is the old model's, which nothing draws once
//! it is re-imported.
//!
//! **A converter that does not write the files itself** (the `.blend` path
//! through Blender, #1072) has its program export into the staging folder and
//! writes the `.glb` and textures from there, so [`swapped_in`] notes its outputs
//! the same way; one that wrote somewhere else would have to call `note_written`
//! on each output once it returns.

use crate::core::fs::written::note_written;
use std::path::Path;

/// Write `bytes` at `path`, a file inside a conversion's staging folder: its
/// final path is noted when the set is swapped in ([`swapped_in`]), not here.
pub(in crate::asset::convert) fn write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    // lint: allow(asset-write) a staged file no cache has seen; `swapped_in` notes its final path
    std::fs::write(path, bytes)
}

/// Note the conversion of `source` now in place: the `.glb` at `out`, and
/// `source` itself.
pub(super) fn swapped_in(source: &Path, out: &Path) {
    note_written(&out.to_string_lossy());
    note_written(&source.to_string_lossy());
}
