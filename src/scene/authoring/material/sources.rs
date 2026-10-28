//! src/scene/authoring/material/sources.rs — read the image files a texture recipe's
//! `image` nodes name (#1491).
//!
//! procgen is sim code and reads no files, so every caller that evaluates a recipe
//! (`Texture.Bake` / `BakeSet` / `Stats` / `Preview`, and a material's maps bake,
//! `Material.DefineAsset` / `Material.Rebake`) loads its sources here first. Each
//! path is taken from the project root, as `require` takes its scripts (#1062): a
//! file outside the project is read, but warns once per file, since it is an input
//! the seed doesn't cover and may be missing on another machine. A file that can't
//! be read or decoded is an error naming the node and the path. Every bake re-reads
//! the files, so a rebake picks up an edited source.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::core::paths::{native, outside_root, shown};
use crate::procgen::{Source, Sources, TextureRecipe};

/// The outside files already warned about, process-wide: the warning is once per file.
static WARNED: Mutex<BTreeSet<PathBuf>> = Mutex::new(BTreeSet::new());

/// Read and decode every file `recipe`'s `image` nodes name, each path taken from
/// `root` (the open project's, [`crate::core::project::root`]). The recipe's
/// structure is checked first ([`crate::procgen::validate`]), so a cycle or an
/// unknown input is reported as itself, before any file is read.
pub fn load_sources(recipe: &TextureRecipe, root: &Path) -> Result<Sources, String> {
    crate::procgen::validate(recipe).map_err(|e| e.to_string())?;
    let mut sources = Sources::new();
    for (node, path) in recipe.image_paths() {
        let file = root.join(native(path));
        let source = std::fs::read(&file)
            .map_err(|e| e.to_string())
            .and_then(|bytes| Source::decode(path, &bytes))
            .map_err(|e| format!("image node '{node}' can't read {path:?}: {e}"))?;
        if let Some(outside) = outside_root(&file, root) {
            warn_once(outside, path);
        }
        sources.insert(path, source);
    }
    Ok(sources)
}

fn warn_once(real: PathBuf, written: &str) {
    let shown = shown(&real);
    let first = WARNED.lock().map_or(true, |mut warned| warned.insert(real));
    if first {
        log::warn!(
            "[Texture] image {written:?} reads {shown}, outside the project: the texture \
             now depends on a file that may not exist on another machine, and the seed \
             does not cover it"
        );
    }
}

#[cfg(test)]
#[path = "sources_tests.rs"]
mod tests;
