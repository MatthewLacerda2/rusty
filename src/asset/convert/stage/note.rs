//! The write log's half of a conversion (#1207, #1219): a converter writes plain
//! files into its staging folder ([`write`]), and once the set is swapped in,
//! [`swapped_in`] notes every file a reader now finds at its real path in
//! `core::fs::written`, so a path-keyed cache (the renderer's textures, its
//! particle meshes) drops what it read from the old conversion.
//!
//! A staged path is never noted: no cache has seen it, and it is gone after the
//! swap. What is noted is the cached `.glb`, every file of the textures folder
//! beside it (a re-conversion with the same digest replaces them one by one,
//! `move_in`), and the **source**, which is what scenes and mesh emitters name
//! (`soldier.fbx::Body`), so a consumer keyed by the source matches too.
//!
//! **A converter that does not write the files itself** (the `.blend` path
//! through Blender, #1072, not built yet) still writes into the staging folder,
//! so [`swapped_in`] notes its outputs the same way; one that wrote somewhere
//! else would have to call `note_written` on each output once it returns.

use crate::core::fs::written::note_written;
use std::path::Path;

/// Write `bytes` at `path`, a file inside a conversion's staging folder: its
/// final path is noted when the set is swapped in ([`swapped_in`]), not here.
pub(in crate::asset::convert) fn write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    // lint: allow(asset-write) a staged file no cache has seen; `swapped_in` notes its final path
    std::fs::write(path, bytes)
}

/// Note the conversion of `source` now in place: the `.glb` at `out`, every file
/// of its `textures` folder, and `source` itself.
pub(super) fn swapped_in(source: &Path, out: &Path, textures: &Path) {
    note_written(&out.to_string_lossy());
    if let Ok(entries) = std::fs::read_dir(textures) {
        for entry in entries.flatten() {
            note_written(&entry.path().to_string_lossy());
        }
    }
    note_written(&source.to_string_lossy());
}
