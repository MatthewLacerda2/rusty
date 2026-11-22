//! A saved scene never stores an extracted texture's cache path: a map is
//! `<source>#tex/<identity>` (`soldier.fbx#tex/Skin`), resolved here, as Unity's
//! materials name a sub-asset. Resolving never converts; a re-conversion notes the
//! source in `core::fs::written`, so caches keyed by a reference drop it by [`source`].

pub mod extracted;
mod map_ref;
mod resolved;

pub use map_ref::MapRef;

use super::converter_for;
use rusty_core::fs::written::Stale;
use std::path::{Path, PathBuf};

const MARK: &str = "#tex/";

pub fn reference(source: &str, identity: &str) -> String {
    format!("{source}{MARK}{identity}")
}

pub fn source(map: &str) -> Option<&str> {
    split(map).map(|(source, _)| source)
}

fn split(map: &str) -> Option<(&str, &str)> {
    let (source, identity) = map.split_once(MARK)?;
    let converted = converter_for(Path::new(source)).is_some();
    (converted && !identity.is_empty()).then_some((source, identity))
}

pub fn file(map: &str) -> Option<PathBuf> {
    let Some((source, identity)) = split(map) else {
        return Some(PathBuf::from(map));
    };
    resolved::file(source, identity)
}

pub fn exists(map: &str) -> bool {
    file(map).is_some_and(|f| f.exists())
}

pub fn is_written(stale: &Stale, map: &str) -> bool {
    let Some((source, identity)) = split(map) else {
        return stale.matches(map);
    };
    let extracted =
        || resolved::file(source, identity).is_some_and(|f| stale.matches(&f.to_string_lossy()));
    stale.matches(map) || stale.matches(source) || extracted()
}

#[cfg(test)]
mod extracted_tests;
#[cfg(test)]
mod resolved_tests;
#[cfg(test)]
mod tests;
