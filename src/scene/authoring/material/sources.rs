//! src/scene/authoring/material/sources.rs — load a texture recipe's files: the
//! graphs its instances name (#1744) and the images its `image` nodes read (#1491).
//!
//! procgen is sim code and reads no files, so every caller that evaluates a recipe
//! (`Texture.Bake` / `BakeSet` / `Stats` / `Preview`, and a material's maps bake,
//! `Material.DefineAsset` / `Material.Rebake`) loads it here first, and evaluates
//! the [`Loaded`] recipe this returns: the graphs are read
//! ([`super::graphs::read_graphs`]) and expanded into one flat recipe
//! ([`crate::procgen::graph::expand`]), which is checked
//! ([`crate::procgen::validate`]) before any image is read. Each path is taken from
//! the project root, as `require` takes its scripts (#1062): a file outside the
//! project is read, but warns once per file, since it is an input the seed doesn't
//! cover and may be missing on another machine. A file that can't be read or
//! decoded is an error naming the node and the path. Every bake re-reads the
//! files, so a rebake picks up an edited graph or source.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::core::paths::{native, outside_root, shown};
use crate::procgen::graph::expand;
use crate::procgen::{Source, Sources, TextureRecipe};

/// The outside files already warned about, process-wide: the warning is once per file.
static WARNED: Mutex<BTreeSet<PathBuf>> = Mutex::new(BTreeSet::new());

/// A recipe ready to run: every graph instance expanded, every image decoded.
#[derive(Debug)]
pub struct Loaded {
    /// The flat recipe to evaluate: the one handed in, its instances expanded.
    pub recipe: TextureRecipe,
    pub sources: Sources,
}

/// Read `recipe`'s graphs and expand them, check the flat recipe, then read and
/// decode every file its `image` nodes name, each path taken from `root` (the open
/// project's, [`crate::core::project::root`]). A cycle or an unknown input is
/// reported as itself, before any image is read.
pub fn load_recipe(recipe: &TextureRecipe, root: &Path) -> Result<Loaded, String> {
    let graphs = super::graphs::read_graphs(recipe, root)?;
    let expanded = expand(recipe, &graphs)?;
    expanded.validate()?;
    let sources = read_images(&expanded.recipe, root)?;
    Ok(Loaded {
        recipe: expanded.recipe,
        sources,
    })
}

/// Decode every file `recipe`'s `image` nodes name.
fn read_images(recipe: &TextureRecipe, root: &Path) -> Result<Sources, String> {
    let mut sources = Sources::new();
    for (node, path) in recipe.image_paths() {
        let file = root.join(native(path));
        let source = std::fs::read(&file)
            .map_err(|e| e.to_string())
            .and_then(|bytes| Source::decode(path, &bytes))
            .map_err(|e| format!("image node '{node}' can't read {path:?}: {e}"))?;
        warn_if_outside(&file, root, "image", path);
        sources.insert(path, source);
    }
    Ok(sources)
}

/// Warn, once per file, when `file` (named `written` by a `kind` node) lies outside
/// `root`.
pub(super) fn warn_if_outside(file: &Path, root: &Path, kind: &str, written: &str) {
    let Some(real) = outside_root(file, root) else {
        return;
    };
    let shown = shown(&real);
    let first = WARNED.lock().map_or(true, |mut warned| warned.insert(real));
    if first {
        log::warn!(
            "[Texture] {kind} {written:?} reads {shown}, outside the project: the texture \
             now depends on a file that may not exist on another machine, and the seed \
             does not cover it"
        );
    }
}

#[cfg(test)]
#[path = "sources_tests.rs"]
mod tests;
