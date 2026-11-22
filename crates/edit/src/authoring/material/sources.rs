//! A file outside the folder its path is named from is read but warns once per file: an input the
//! seed doesn't cover, possibly missing on another machine. The engine's own content never warns.
//! An image is decoded only when the node that reads it runs, and freed after its last reader.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use rusty_core::paths::{outside_root, shown};
use rusty_core::project::ContentRoots;
use rusty_procgen::graph::expand;
use rusty_procgen::{Source, Sources, TextureRecipe};

static WARNED: Mutex<BTreeSet<PathBuf>> = Mutex::new(BTreeSet::new());

#[derive(Debug)]
pub struct Loaded {
    pub recipe: TextureRecipe,
    pub sources: Sources,
}

/// Evaluate only what this returns: procgen reads no files, so the graphs and images load here.
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
