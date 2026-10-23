//! A material map naming a texture a conversion extracted (#1130).
//!
//! An FBX's embedded texture is written into the import cache, at a path that
//! carries the conversion's digest and a `.1` suffix when another texture of the
//! same file name came first. A saved scene never stores that path: an import
//! names such a map `<source>#tex/<identity>` (`soldier.fbx#tex/Skin`), the
//! identity being the one the converter gave the texture (the FBX element's name,
//! `fbx::texture::identity`), and whatever opens the file resolves it here. Unity
//! keeps the same split: a material names a sub-asset of the model, not a file.
//!
//! Resolving reads the cache as the last import left it and never converts: the
//! model's own import (instancing it, or a scene load re-importing its meshes)
//! keeps the cache current, and a re-conversion notes the source in
//! `core::fs::written`, so a cache keyed by a reference drops it by its
//! [`source`]. A map that is not a reference is the path it is.

use super::{cache_location, converter_for, glb};
use crate::asset::gltf::uri;
use std::path::{Path, PathBuf};

/// What separates the source from the identity.
const MARK: &str = "#tex/";

/// The reference to the texture `identity` extracted from the source at `source`.
pub fn reference(source: &str, identity: &str) -> String {
    format!("{source}{MARK}{identity}")
}

/// The source a reference names, `None` when `map` is a plain path: only a
/// converted format's extension before the mark makes one.
pub fn source(map: &str) -> Option<&str> {
    split(map).map(|(source, _)| source)
}

fn split(map: &str) -> Option<(&str, &str)> {
    let (source, identity) = map.split_once(MARK)?;
    let converted = converter_for(Path::new(source)).is_some();
    (converted && !identity.is_empty()).then_some((source, identity))
}

/// The file the material map `map` names: a reference resolved through its
/// source's import cache, any other map the path it is. `None` when a
/// reference names no texture the cache holds.
pub fn file(map: &str) -> Option<PathBuf> {
    let Some((source, identity)) = split(map) else {
        return Some(PathBuf::from(map));
    };
    let glb = cache_location(Path::new(source)).ok()?;
    // The folder the glTF importer anchors the same URI at.
    let base_dir = glb.parent().unwrap_or_else(|| Path::new(""));
    uri::file(base_dir, &glb::image_uri(&glb, identity)?)
}

#[cfg(test)]
mod tests;
