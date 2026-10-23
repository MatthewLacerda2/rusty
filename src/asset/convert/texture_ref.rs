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

mod map_ref;
mod resolved;

pub use map_ref::MapRef;

use super::converter_for;
use crate::core::fs::written::Stale;
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
/// reference names no texture the cache holds. A source's textures are read
/// once and kept until it is converted again ([`resolved`]).
pub fn file(map: &str) -> Option<PathBuf> {
    let Some((source, identity)) = split(map) else {
        return Some(PathBuf::from(map));
    };
    resolved::file(source, identity)
}

/// Whether the file `map` names exists: what a cache keyed by maps re-checks a
/// miss with (`Freshness::poll_with`).
pub fn exists(map: &str) -> bool {
    file(map).is_some_and(|f| f.exists())
}

/// Whether the map `map` names a file in `stale`: the file itself, or a texture
/// extracted from a source that was converted again.
pub fn is_written(stale: &Stale, map: &str) -> bool {
    stale.matches(map) || source(map).is_some_and(|s| stale.matches(s))
}

#[cfg(test)]
mod resolved_tests;
#[cfg(test)]
mod tests;
