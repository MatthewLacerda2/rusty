//! src/scene/authoring/fields/mod.rs — the field table (#1369): every serialized
//! field of every first-class component, and the authoring op that writes it.
//!
//! One table, two readers: `Component.Set` (`api::component`) writes a Lua table's
//! fields through it, and a scene or prefab load holds what it read through it
//! ([`hold`], #1418), so a value the inspector's setters would refuse never reaches
//! the World from a file either. It lives here, not in `api`, because loading is
//! `scene`'s and `scene` sits below `api` in the layer table.

mod hold;
mod introspect;
mod kinds;
pub(crate) mod table;
pub(crate) mod value;
mod write;

#[cfg(test)]
mod hold_tests;
#[cfg(test)]
mod tests;

pub use hold::{hold_entity, hold_world};
pub use kinds::{find, KINDS};
pub use table::{FieldInfo, Kind};
pub use write::Skip;
