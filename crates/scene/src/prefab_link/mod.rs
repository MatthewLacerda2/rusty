//! stored-format: the `.prefab` links a scene saves (a removed part's node path in `removed_parts`)

pub mod instance;
pub mod removed;

use std::path::Path;

pub const PREFAB_EXTENSION: &str = "prefab";

pub fn is_prefab_path(path: &str) -> bool {
    Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case(PREFAB_EXTENSION))
}

/// The local id of a model instance's entity whose node the model's current import
/// lost: no source entity has it, so propagation destroys the entity.
pub const GONE: u32 = u32::MAX;

pub fn is_model_source(source: &str) -> bool {
    !is_prefab_path(source) && rusty_asset::is_model_path(source)
}
