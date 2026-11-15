//! crates/edit/src/authoring/material/sources.rs — load a texture recipe's files: the
//! graphs its instances name (#1744) and the images its `image` nodes read (#1491).
//!
//! procgen is sim code and reads no files, so every caller that evaluates a recipe
//! (`Texture.Bake` / `BakeSet` / `Stats` / `Preview`, and a material's maps bake,
//! `Material.DefineAsset` / `Material.Rebake`) loads it here first, and evaluates
//! the [`Loaded`] recipe this returns: the graphs are read
//! ([`super::graphs::read_graphs`]) and expanded into one flat recipe
//! ([`rusty_procgen::graph::expand`]), which is checked
//! ([`rusty_procgen::validate`]) before any image is read. Each path is taken from
//! the project root, as `require` takes its scripts (#1062), except an `engine/…`
//! one, which is taken from the engine's content folder (#2185,
//! [`ContentRoots`]): a file outside the folder its path is named from is read, but
//! warns once per file, since it is an input the seed doesn't cover and may be
//! missing on another machine. The engine's own content ships beside every
//! install, so a file inside the engine's content folder never warns. A file that
//! isn't there is an error naming the node and the path, before anything runs. An
//! image is read and decoded only when the node that reads it runs, and freed after
//! its last reader (#2002), so one that can't be decoded is the bake's error then.
//! Every bake re-reads the files, so a rebake picks up an edited graph or source.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use rusty_core::paths::{outside_root, shown};
use rusty_core::project::ContentRoots;
use rusty_procgen::graph::expand;
use rusty_procgen::{Source, Sources, TextureRecipe};

/// The outside files already warned about, process-wide: the warning is once per file.
static WARNED: Mutex<BTreeSet<PathBuf>> = Mutex::new(BTreeSet::new());

/// A recipe ready to run: every graph instance expanded, every image file found
/// (and decoded when a node reads it).
#[derive(Debug)]
pub struct Loaded {
    /// The flat recipe to evaluate: the one handed in, its instances expanded.
    pub recipe: TextureRecipe,
    pub sources: Sources,
}

/// Read `recipe`'s graphs and expand them, check the flat recipe, then read and
/// find every file its `image` nodes name and read every font its `text` nodes
/// name (`super::fonts`), each path taken from `roots` (the open project's,
/// [`rusty_core::project::root`], and the engine's). A cycle or an unknown input
/// is reported as itself, before any image is read.
pub fn load_recipe(recipe: &TextureRecipe, roots: &ContentRoots) -> Result<Loaded, String> {
    let graphs = super::graphs::read_graphs(recipe, roots)?;
    let expanded = expand(recipe, &graphs)?;
    expanded.validate()?;
    let mut sources = read_images(&expanded.recipe, roots)?;
    super::fonts::read_fonts(&expanded.recipe, roots, &mut sources)?;
    Ok(Loaded {
        recipe: expanded.recipe,
        sources,
    })
}

/// Find every file `recipe`'s `image` nodes name, and hand them to the runner to
/// read and decode when a node needs one.
fn read_images(recipe: &TextureRecipe, roots: &ContentRoots) -> Result<Sources, String> {
    let mut paths = Vec::new();
    for (node, path) in recipe.image_paths() {
        let file = roots.file(path);
        if !file.is_file() {
            return Err(format!(
                "image node '{node}' can't read {path:?}: no such file"
            ));
        }
        warn_if_outside(roots, "image", path);
        paths.push(path.to_string());
    }
    let roots = roots.clone();
    let loader = Arc::new(move |path: &str| {
        let bytes = std::fs::read(roots.file(path)).map_err(|e| e.to_string())?;
        Source::decode(path, &bytes)
    });
    Ok(Sources::lazy(paths, loader))
}

/// Warn, once per file, when the file `written` names (by a `kind` node) lies
/// outside the folder that path is named from ([`ContentRoots::base`]). Returns
/// whether it lies outside, warned already or not.
pub(super) fn warn_if_outside(roots: &ContentRoots, kind: &str, written: &str) -> bool {
    let engine = rusty_core::project::engine_relative(written).is_some();
    let folder = if engine {
        "engine's content folder"
    } else {
        "project"
    };
    let Some(real) = outside_root(&roots.file(written), roots.base(written)) else {
        return false;
    };
    let shown = shown(&real);
    let first = WARNED.lock().map_or(true, |mut warned| warned.insert(real));
    if first {
        log::warn!(
            "[Texture] {kind} {written:?} reads {shown}, outside the {folder}: the texture \
             now depends on a file that may not exist on another machine, and the seed \
             does not cover it"
        );
    }
    true
}

#[cfg(test)]
#[path = "sources_tests.rs"]
mod tests;
