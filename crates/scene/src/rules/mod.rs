//! The bounds a stored value is held to, below `edit::authoring`: the authoring ops store
//! through them, and the base holds a value that skipped them (a scene file, a prefab) by the
//! same functions, so each bound lives once.

pub mod joint;
pub mod lens;
pub mod material;
pub mod mesh;
pub mod sanitize;
pub mod taa;
