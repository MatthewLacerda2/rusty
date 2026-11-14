//! crates/scene/src/prefab_link/mod.rs — what the base knows of a GameObject's
//! `PrefabLink`: which entities make up a linked instance (`instance`), what a
//! link's source is, and the part a destroy takes out of a model instance
//! (`removed`, Unity's *Removed GameObject* override).
//!
//! The prefab verbs (`edit::prefab`: extract, stamp, propagate, apply, revert)
//! build on these, and `prefab` re-exports `instance`, the extension and the
//! source tests under its own paths (`prefab::source`); `removed` is named here
//! only. They sit below that layer because the base reaches them in live code (a
//! destroy records the part it removed).
//!
//! stored-format: the `.prefab` links a scene saves (a removed part's node path in `removed_parts`)

pub mod instance;
pub mod removed;

use std::path::Path;

/// The one standardised prefab file extension (JSON inside) — a distinct extension
/// from `.scene` so the inspector dispatch recognises it cleanly.
pub const PREFAB_EXTENSION: &str = "prefab";

/// True if `path` looks like a prefab file (`.prefab`).
pub fn is_prefab_path(path: &str) -> bool {
    Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case(PREFAB_EXTENSION))
}

/// The local id an entity of a model instance takes when its node is gone from the
/// model's current import: no source entity has it, so propagation destroys the
/// entity (`prefab::structure`, #1665).
pub const GONE: u32 = u32::MAX;

/// Whether `source` (a link's source path) is a model rather than a `.prefab`.
pub fn is_model_source(source: &str) -> bool {
    !is_prefab_path(source) && rusty_asset::is_model_path(source)
}
